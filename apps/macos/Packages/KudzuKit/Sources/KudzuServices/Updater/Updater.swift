import Foundation
import KudzuCore

/// Self-updates. Port of `updater.rs` (and the parts of tauri-plugin-updater it relied on).
///
/// Flow: check → download quietly (signature verified) → hold the verified package in memory →
/// install when the user quits, or immediately via "Restart to Update". Kudzu never restarts on
/// its own: a text expander restarting mid-sentence would drop the trigger being typed.
///
/// Checks run 20 s after launch, then every 6 h while **Check for updates automatically** is on
/// (release builds only).
public final class Updater: @unchecked Sendable {
    // @unchecked: `current`, `pending` and `busy` are only touched while holding `lock`.

    public static let firstCheckDelay: Duration = .seconds(20)
    public static let checkInterval: Duration = .seconds(6 * 3600)
    /// `plugins.updater.pubkey` of tauri.conf.json: every installed copy trusts only this key.
    public static let publicKeyBase64 = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDZFNTIyNjlGNDI0MUYwMDcKUldRSDhFRkNueVpTYnVNNUJHYWt0dWV4Ti83MHA5UTFaUEN3d050NWlsdlBrRG5uNTdrZUFJT0MK"
    public static let devMessage = "Updates are unavailable in Kudzu Dev."

    /// What `fetch` returns: the feed, or the archive bytes (tests substitute it).
    public typealias Fetch = @Sendable (_ url: URL, _ progress: @escaping @Sendable (UInt64, UInt64?) -> Void) async throws -> Data

    struct Pending {
        var version: String
        var archive: Data
    }

    private let lock = NSLock()
    private var current: UpdateState = .idle
    private var pending: Pending?
    private var busy = false

    private let identity: AppIdentity
    private let publicKey: String
    private let channel: @Sendable () -> UpdateChannel
    private let autoCheck: @Sendable () -> Bool
    private let publish: @Sendable (UpdateState) -> Void
    private let fetch: Fetch
    private let bundle: URL?
    private let log = Logging.logger(category: "updater")
    private var loop: Task<Void, Never>?

    public init(
        identity: AppIdentity,
        publicKey: String = Updater.publicKeyBase64,
        bundle: URL? = Installer.bundleURL,
        channel: @escaping @Sendable () -> UpdateChannel,
        autoCheck: @escaping @Sendable () -> Bool,
        publish: @escaping @Sendable (UpdateState) -> Void,
        fetch: Fetch? = nil
    ) {
        self.identity = identity
        self.publicKey = publicKey
        self.bundle = bundle
        self.channel = channel
        self.autoCheck = autoCheck
        self.publish = publish
        self.fetch = fetch ?? { url, progress in try await Downloader.fetch(url, progress: progress) }
    }

    /// `update_allowed`: Kudzu Dev can't install production updates.
    public static func updateAllowed(_ identifier: String) -> Result<Void, ServiceError> {
        AppIdentity.isDevIdentifier(identifier) ? .failure(ServiceError(devMessage)) : .success(())
    }

    /// Current state, for windows that open after a check finished.
    public var state: UpdateState { lock.withLock { current } }

    private func set(_ next: UpdateState) {
        lock.withLock { current = next }
        publish(next)
    }

    /// Starts the background check loop, except in debug builds and Kudzu Dev.
    public func start() {
        // This version started and kept running: the copy an update replaced can go.
        if let bundle {
            Task.detached(priority: .background) {
                try? await Task.sleep(for: Updater.firstCheckDelay)
                Installer.removePreviousVersion(beside: bundle)
            }
        }
        #if DEBUG
            return
        #else
            guard case .success = Self.updateAllowed(identity.bundleID) else { return }
            loop = Task.detached(priority: .utility) { [weak self] in
                try? await Task.sleep(for: Updater.firstCheckDelay)
                while !Task.isCancelled {
                    guard let self else { return }
                    if self.autoCheck() { _ = await self.check(quiet: true) }
                    try? await Task.sleep(for: Updater.checkInterval)
                }
            }
        #endif
    }

    public func stop() { loop?.cancel() }

    // MARK: Checking

    /// Checks the feed and, when there's a newer version, downloads and verifies it in the
    /// background. `quiet` keeps scheduled checks from surfacing transient network errors.
    @discardableResult
    public func check(quiet: Bool) async -> UpdateState {
        if case let .failure(e) = Self.updateAllowed(identity.bundleID) {
            let next = UpdateState.error(message: e.message, needsMove: false)
            set(next)
            return next
        }
        let started = lock.withLock { () -> Bool in
            if busy { return false } // a check or download is already running
            busy = true
            return true
        }
        guard started else { return state }
        defer { lock.withLock { busy = false } }

        switch state {
        case .ready, .restarting: return state // already holding an update; anything newer can wait for next launch
        default: break
        }
        if let problem = Installer.locationProblem(bundle: bundle) {
            let next = UpdateState.error(message: problem, needsMove: true)
            set(next)
            return next
        }
        if !quiet { set(.checking) }

        let feed: UpdateFeed
        let platform: UpdateFeed.Platform
        do {
            let data = try await fetch(UpdateFeed.url(for: channel()), { _, _ in })
            feed = try UpdateFeed.parse(data)
            guard feed.isNewer(than: identity.version) else {
                let next = UpdateState.upToDate(checkedAt: KudzuDate.format(Date()))
                set(next)
                return next
            }
            guard let p = feed.platform() else {
                throw ServiceError("the platform `darwin-\(UpdateFeed.currentArch)` was not found on the response `platforms` object")
            }
            platform = p
        } catch {
            let message = error.localizedDescription
            log.warning("update check failed: \(message)")
            let next = quiet ? state : UpdateState.error(message: message, needsMove: false)
            set(next)
            return next
        }

        // Download (and verify) in the background.
        let version = feed.version
        let percent = Percent()
        do {
            let archive = try await fetch(platform.url) { [weak self] downloaded, total in
                // One event per whole percent, not per network chunk.
                guard let self, percent.advance(downloaded: downloaded, total: total) else { return }
                self.set(.downloading(version: version, downloaded: downloaded, total: total))
            }
            try Self.verify(archive, signature: platform.signature, publicKey: publicKey, fileVersion: version)
            lock.withLock { pending = Pending(version: version, archive: archive) }
            let next = UpdateState.ready(version: version, notes: feed.notes, date: feed.pubDate)
            set(next)
            return next
        } catch {
            let message = (error as? Minisign.Failure)?.description ?? error.localizedDescription
            log.warning("update download failed: \(message)")
            let next = quiet ? UpdateState.idle : UpdateState.error(message: message, needsMove: false)
            set(next)
            return next
        }
    }

    /// Verifies a downloaded archive against the feed's signature and the embedded key.
    /// `fileVersion`, when set, requires the trusted comment to name this archive
    /// (`file:Kudzu_<version>_`). Omitted, the comment is checked only by its signature,
    /// which is what the existing verifier tests do.
    public static func verify(_ archive: Data, signature: String, publicKey: String, fileVersion: String? = nil) throws(Minisign.Failure) {
        let key = try Minisign.PublicKey(tauriBase64: publicKey)
        let sig = try Minisign.Signature(tauriBase64: signature)
        if let fileVersion, !sig.trustedComment.contains("file:Kudzu_\(fileVersion)_") {
            throw Minisign.Failure.invalidTrustedComment
        }
        try Minisign.verify(archive, signature: sig, publicKey: key)
    }

    // MARK: Installing

    /// Installs the waiting update. Returns true when something was installed.
    @discardableResult
    public func installPending() throws -> Bool {
        if case let .failure(e) = Self.updateAllowed(identity.bundleID) { throw e }
        guard let p = lock.withLock({ () -> Pending? in defer { pending = nil }; return pending }) else { return false }
        set(.restarting(version: p.version))
        do {
            guard let bundle else { throw ServiceError("Run the installed Kudzu application to update it.") }
            try Installer.install(archive: p.archive, over: bundle, expectedVersion: p.version, currentVersion: identity.version)
            return true
        } catch {
            let message = error.localizedDescription
            log.error("update install failed: \(message)")
            set(.error(message: message, needsMove: Installer.locationProblem(bundle: bundle) != nil))
            throw ServiceError(message)
        }
    }

    /// "Restart to Update": install now and relaunch. `terminate` quits the app afterwards.
    public func restartToUpdate(terminate: @escaping @Sendable @MainActor () -> Void) async throws {
        guard try installPending() else { return }
        // A beat so the UI can show "Restarting…".
        try? await Task.sleep(for: .milliseconds(400))
        if let bundle { try Installer.relaunch(bundle) }
        await terminate()
    }

    /// Called on quit: if an update is waiting, swap it in so the next launch is the new version.
    public func installOnQuit() {
        guard case .success = Self.updateAllowed(identity.bundleID) else { return }
        do {
            if try installPending() { log.info("installed update on quit") }
        } catch {
            log.error("couldn't install update on quit: \(error.localizedDescription)")
        }
    }
}

/// Download progress throttled to whole percents (or whole MiB without a length).
private final class Percent: @unchecked Sendable {
    // @unchecked: only called from the downloader's serial delegate queue.
    private var last = UInt64.max
    func advance(downloaded: UInt64, total: UInt64?) -> Bool {
        let percent = total.map { downloaded * 100 / max($0, 1) } ?? (downloaded >> 20)
        guard percent != last else { return false }
        last = percent
        return true
    }
}
