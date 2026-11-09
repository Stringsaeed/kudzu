import Foundation
import KudzuCore

// The macOS engine runtime: tap handling, insertion worker, status poll. Port of `macos/engine.rs`.
//
// Threads:
// - **tap** (`kudzu-event-tap`): runs the CGEventTap callback. Matching happens here, under the
//   `core` lock, because the callback must decide synchronously whether to swallow the key.
// - **worker** (`kudzu-engine-worker`): renders and inserts. Everything slow (sleeps, paste,
//   app activation, host callbacks for forms/expansions) happens here.
// - **poll** (`kudzu-engine-poll`): permissions, Secure Input and tap attachment every second;
//   enforces the 1 s hold deadline every 100 ms.
//
// Host callbacks are never invoked while `core` is locked.

/// The engine runtime (Rust `Imp`).
///
/// `Sendable` without `@unchecked`: every mutable piece lives in a ``Locked`` box or the
/// ``JobQueue``, both of which serialize access themselves.
final class EngineRuntime: Sendable {
    let host: any EngineHost
    let core = Locked(Core())
    /// Test hook, called after the snapshot is built and before it is installed.
    /// The argument is the revision, or 0 for an unversioned reload.
    let beforeLibraryInstall = Locked<(@Sendable (UInt64) -> Void)?>(nil)
    let flags = Locked(Flags())
    let tap = Locked<TapHandle?>(nil)
    let jobs = JobQueue()
    let lastStatus = Locked<EngineStatus?>(nil)
    let ownPid = ProcessInfo.processInfo.processIdentifier

    init(host: any EngineHost) {
        self.host = host
        let permissions = AX.permissions()
        let postEvent = AX.postEventAllowed()
        let secure = AX.secureInput().on
        flags.withLock { flags in
            flags.postEvent = postEvent
            flags.secureInput = secure
            flags.inputMonitoring = permissions.inputMonitoring
        }
        let queue = jobs
        let worker = Thread { [weak self] in
            while let job = queue.next() {
                guard let runtime = self else { break }
                autoreleasepool { runtime.run(job) }
            }
        }
        worker.name = "kudzu-engine-worker"
        worker.qualityOfService = .userInitiated
        worker.start()
    }

    // MARK: - Public surface

    func start() {
        let alreadyRunning = flags.withLock { flags in
            defer { flags.running = true }
            return flags.running
        }
        if alreadyRunning {
            return
        }
        autoreleasepool { refresh() }
        let epoch = flags.withLock { flags in
            flags.pollEpoch &+= 1
            return flags.pollEpoch
        }
        let poll = Thread { [weak self] in
            var tick: UInt32 = 0
            while true {
                Thread.sleep(forTimeInterval: RuntimeTiming.pollTick)
                guard let runtime = self else { break }
                let current = runtime.flags.withLock { $0.pollEpoch == epoch && $0.running }
                if !current {
                    break
                }
                tick &+= 1
                if tick.isMultiple(of: RuntimeTiming.statusEveryTicks) {
                    autoreleasepool { runtime.refresh() }
                }
                runtime.enforceHoldDeadline()
            }
        }
        poll.name = "kudzu-engine-poll"
        poll.qualityOfService = .utility
        poll.start()
    }

    func stop() {
        flags.withLock { flags in
            flags.running = false
            flags.pollEpoch &+= 1
        }
        let old = tap.swap(nil)
        old?.stop()
        flags.withLock { $0.tapMode = .off }
        invalidate()
        Pasteboard.restorePending()
        publishStatus()
    }

    /// Stops everything and lets the worker exit (engine dropped).
    func shutdown() {
        stop()
        jobs.close()
    }

    /// `revision` nil always installs and does not lower `appliedRevision`. A revision older
    /// than the one already installed is dropped, including when this call was already building
    /// its index. A same-contents reload still records the newer revision so a late older build
    /// cannot apply afterwards, and it does not clear the key buffer.
    func reload(_ library: EngineLibrary, revision: UInt64? = nil) {
        let new = LibrarySnapshot(library, fallbackLocale: System.locale)
        beforeLibraryInstall.get()?(revision ?? 0)
        // Built here, not on the tap thread, and not under the lock.
        let index = TriggerIndex(snippets: library.snippets, groups: library.groups, config: new.config)
        core.withLock { core in
            if let revision {
                if revision < core.appliedRevision { return }
                core.appliedRevision = revision
            }
            // Usage saves after every expansion can trigger reloads. Rebuilding would clear the live
            // key buffer and undo, dropping whatever was typed since, so skip no-op reloads.
            if core.library.hasSameContents(as: new) { return }
            core.matcher.replaceIndex(index)
            core.library = new
            core.undoCaretLeft = 0
        }
    }

    func library() -> LibrarySnapshot {
        core.withLock { $0.library }
    }

    func setPaused(_ paused: Bool) {
        flags.withLock { $0.paused = paused }
        invalidate()
        publishStatus()
    }

    func setSuppressed(_ suppressed: Bool) {
        flags.withLock { $0.suppressed = suppressed }
        core.withLock { $0.resetMatcher() }
    }

    func submitForm(token: UInt64, values: [String: String]) {
        send(.submit(token: token, values: values))
    }

    func cancelForm(token: UInt64) {
        core.withLock { core in
            if core.pendingForm?.token == token {
                core.pendingForm = nil
            }
        }
    }

    func insertSnippet(id: UUID, target: AppInfo?) {
        send(.insert(snippetId: id, target: target))
    }

    // MARK: - Status

    func currentStatus() -> EngineStatus {
        let permissions = AX.permissions()
        let (secureInput, ownerPid) = AX.secureInput()
        let state = flags.get()
        return EngineStatusBuilder.build(StatusInputs(
            paused: state.paused,
            tap: state.tapMode,
            permissions: permissions,
            postEvent: AX.postEventAllowed(),
            secureInput: secureInput,
            secureInputOwner: ownerPid.flatMap(Apps.name(of:))
        ))
    }

    func publishStatus() {
        let status = autoreleasepool { currentStatus() }
        let changed = lastStatus.withLock { last in
            if last == status {
                return false
            }
            last = status
            return true
        }
        if changed {
            host.statusChanged(status)
        }
    }

    /// Permission/Secure Input poll; attaches or upgrades the tap.
    func refresh() {
        let permissions = AX.permissions()
        let postEvent = AX.postEventAllowed()
        let secure = AX.secureInput().on
        let (changed, running) = flags.withLock { flags in
            let wasSecure = flags.secureInput
            let couldPost = flags.postEvent
            let couldListen = flags.inputMonitoring
            flags.secureInput = secure
            flags.postEvent = postEvent
            flags.inputMonitoring = permissions.inputMonitoring
            let changed = wasSecure != secure || (couldPost && !postEvent) || (couldListen && !permissions.inputMonitoring)
            return (changed, flags.running)
        }
        if changed {
            invalidate()
        }
        if running {
            attachTap(accessibility: permissions.accessibility, inputMonitoring: permissions.inputMonitoring)
        }
        publishStatus()
    }

    private func tapHandler(_ mode: TapMode) -> TapHandler {
        { [weak self] kind, event in
            self?.receive(kind, event, mode: mode) ?? false
        }
    }

    private func attachTap(accessibility: Bool, inputMonitoring: Bool) {
        let desired: TapMode = accessibility ? .active : .listenOnly
        enum Outcome { case unchanged, detached, attached }
        let outcome: Outcome = tap.withLock { slot in
            let current = slot?.mode
            if !accessibility && !inputMonitoring {
                guard let old = slot else { return .unchanged }
                old.stop()
                slot = nil
                return .detached
            }
            if current == desired {
                return .unchanged
            }
            var created = TapHandle.start(desired, handler: tapHandler(desired))
            if created == nil && current == nil && desired == .active && inputMonitoring {
                created = TapHandle.start(.listenOnly, handler: tapHandler(.listenOnly))
            }
            guard let new = created else { return .unchanged }
            // Stop the old tap before enabling the new one so no key is matched twice.
            slot?.stop()
            new.enable()
            let mode = new.mode
            flags.withLock { $0.tapMode = mode }
            slot = new
            return .attached
        }
        switch outcome {
        case .unchanged:
            break
        case .detached:
            flags.withLock { $0.tapMode = .off }
            invalidate()
        case .attached:
            invalidate()
        }
    }

    // MARK: - Guards

    func explicitAllowed() -> Bool {
        flags.withLock { !$0.paused && !$0.secureInput && $0.postEvent }
    }

    /// Typed triggers additionally never fire in our own windows (palette, form, editor).
    func typedAllowed() -> Bool {
        flags.withLock { !$0.paused && !$0.secureInput && $0.postEvent && !$0.suppressed }
    }

    func allowed(_ guardKind: InsertGuard) -> Bool {
        switch guardKind {
        case .typed: typedAllowed() && Apps.frontmostPid() != ownPid
        case .explicit: explicitAllowed()
        }
    }

    func isCurrent(_ generation: UInt64) -> Bool {
        core.withLock { $0.generation == generation }
    }

    func enforceHoldDeadline() {
        let expired = core.withLock { core in core.holdExpired ? core.invalidate() : [] }
        for held in expired {
            held.post(Keys.syntheticMarker)
        }
    }

    func invalidate() {
        let drained = core.withLock { $0.invalidate() }
        // Never leave physical input trapped after focus, permission or pause changes.
        for held in drained {
            held.post(Keys.syntheticMarker)
        }
    }

    func send(_ job: Job) {
        jobs.send(job)
    }
}
