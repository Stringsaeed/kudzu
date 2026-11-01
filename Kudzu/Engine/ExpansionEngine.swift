import AppKit
import ApplicationServices
import KudzuCore
import Observation
import OSLog

/// Watches typing system-wide and expands snippets from the store.
@MainActor @Observable
final class ExpansionEngine {
    /// When true, no expansions happen (menu bar "Pause").
    var isPaused = false { didSet { invalidateContext(); updateStatus() } }
    /// True while macOS Secure Input blocks event monitoring.
    private(set) var secureInputActive = false
    /// App that enabled Secure Input, when known.
    private(set) var secureInputOwner: String?
    /// One-line status for the menu bar.
    private(set) var statusText = "Not started"

    private let store: KudzuStore
    private let permissions: Permissions
    private let tap = EventTap()
    private let inserter = Inserter()
    private let secureInput = SecureInputMonitor()
    private let frontmost = FrontmostApp()
    private var matcher = ExpansionMatcher()
    private var snippets: [UUID: SnippetRecord] = [:]
    private var groups: [GroupRecord] = []
    private var config: MatchingConfig?
    private var pendingUndo: (match: TriggerMatch, insertedCount: Int)?
    private var capturingSelection = false
    private var listenOnlyInterrupted = false
    private var held = HoldReplayQueue<CGEvent>()
    private var poll: Task<Void, Never>?
    private var changes: Task<Void, Never>?
    private var insertion: Task<Void, Never>?
    private var deadline: Task<Void, Never>?
    private var generation = 0
    private var undoCaretLeftCount = 0
    private var terminationObserver: NSObjectProtocol?
    private var settingsObserver: NSObjectProtocol?
    private static let log = Logger(subsystem: "sh.saeed.kudzu", category: "Engine")

    init(store: KudzuStore, permissions: Permissions) {
        self.store = store
        self.permissions = permissions
        tap.onKey = { [weak self] event, signal, replay in
            self?.receive(event, signal: signal, replay: replay) ?? false
        }
        tap.onSignal = { [weak self] _ in self?.invalidateContext() }
        frontmost.onActivation = { [weak self] in self?.invalidateContext() }
    }

    isolated deinit { stop() }

    /// Starts listening once permissions allow it. Safe to call repeatedly.
    func start() {
        guard poll == nil else { return }
        reload()
        let stream = store.changes
        changes = Task { [weak self] in
            for await _ in stream {
                guard !Task.isCancelled else { break }
                self?.reload()
            }
        }
        terminationObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.willTerminateNotification, object: nil, queue: .main
        ) { [weak self] _ in MainActor.assumeIsolated { self?.stop() } }
        settingsObserver = NotificationCenter.default.addObserver(
            forName: AppMatchingSettings.didChangeNotification, object: nil, queue: .main
        ) { [weak self] _ in MainActor.assumeIsolated { self?.reload() } }
        refresh()
        poll = Task { [weak self] in
            while !Task.isCancelled {
                do { try await Task.sleep(for: .seconds(1)) } catch { break }
                self?.refresh()
            }
        }
    }

    /// Stops listening and restores any pending clipboard.
    func stop() {
        poll?.cancel(); poll = nil
        changes?.cancel(); changes = nil
        invalidateContext()
        tap.stop()
        inserter.restorePasteboard()
        if let terminationObserver { NotificationCenter.default.removeObserver(terminationObserver) }
        terminationObserver = nil
        if let settingsObserver { NotificationCenter.default.removeObserver(settingsObserver); self.settingsObserver = nil }
    }

    private func reload() {
        do {
            let library = try store.library()
            let records = Dictionary(library.snippets.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
            let newConfig = AppMatchingSettings.shared.matchingConfig
            // Usage saves after every expansion also emit changes. Rebuilding would clear the
            // live key buffer and undo, dropping whatever was typed since, so skip no-op reloads.
            guard records != snippets || library.groups != groups || newConfig != config else { return }
            snippets = records
            groups = library.groups
            config = newConfig
            matcher.reload(snippets: library.snippets, groups: groups, config: newConfig)
            pendingUndo = nil
            undoCaretLeftCount = 0
        } catch { Self.log.error("Library reload failed: \(error)") }
    }

    private func refresh() {
        permissions.refresh()
        secureInput.refresh()
        let changed = secureInputActive != secureInput.isEnabled
        secureInputActive = secureInput.isEnabled
        secureInputOwner = secureInput.responsibleApp
        if changed || !permissions.postEvent { invalidateContext() }
        #if APPSTORE
        let desired: ExpansionTapMode = .listenOnly
        #else
        let desired: ExpansionTapMode = permissions.accessibility ? .active : .listenOnly
        #endif
        if desired == .active || permissions.inputMonitoring {
            if !tap.start(mode: desired), permissions.inputMonitoring { tap.start(mode: .listenOnly) }
        } else { tap.stop() }
        updateStatus()
    }

    private func updateStatus() {
        if isPaused { statusText = "Paused" }
        else if secureInputActive { statusText = "Secure Input on — expansion blocked" }
        else if tap.mode == nil { statusText = "Needs Input Monitoring" }
        else if !permissions.postEvent { statusText = "Needs Accessibility to type" }
        else if tap.mode == .listenOnly { statusText = "Listening (limited — grant Accessibility for best reliability)" }
        else { statusText = "Listening" }
    }

    private var allowed: Bool {
        // A key non-activating panel (palette, form) receives typing while another app stays
        // frontmost, so the frontmost check alone would expand triggers typed into our own panels.
        !capturingSelection && !isPaused && !secureInputActive && permissions.postEvent &&
        NSApp.keyWindow == nil &&
        NSWorkspace.shared.frontmostApplication?.processIdentifier != ProcessInfo.processInfo.processIdentifier
    }

    private func resetMatcher() {
        _ = matcher.handle(.reset, bundleID: frontmost.bundleID)
        undoCaretLeftCount = 0
        pendingUndo = nil
    }

    private func invalidateContext() {
        generation += 1
        insertion?.cancel(); insertion = nil
        deadline?.cancel(); deadline = nil
        resetMatcher()
        // Never leave physical input trapped after focus, permissions or pause changes.
        for event in held.drain() { post(event, marker: EventTap.syntheticMarker) }
    }

    private func receive(_ event: CGEvent, signal: EventTap.Signal?, replay: Bool) -> Bool {
        if !replay, tap.mode == .active, held.state == .inserting, let copy = event.copy() {
            return held.enqueue(copy)
        }
        // Listen-only can't withhold keys. Anything typed before our backspaces go out would
        // land inside the deletion range, so the pending expansion is abandoned instead.
        if !replay, tap.mode == .listenOnly, insertion != nil, signal != nil {
            listenOnlyInterrupted = true
            return false
        }
        guard allowed else {
            resetMatcher()
            if replay { scheduleReplay() }
            return false
        }
        guard let signal else {
            if replay { scheduleReplay() }
            return false
        }
        let input: KeyInput
        var typed: String?
        switch signal {
        case .character(let text): input = .character(text); typed = text
        case .backspace: input = .backspace
        case .reset: input = .reset
        }
        pendingUndo = nil
        let oldCaret = undoCaretLeftCount
        undoCaretLeftCount = 0
        if let action = matcher.handle(input, bundleID: frontmost.bundleID) {
            let mode = tap.mode ?? .listenOnly
            begin(action, mode: mode, undoCaret: oldCaret, swallowedText: typed)
            return mode == .active
        }
        if replay { scheduleReplay() }
        return false
    }

    private func begin(_ action: MatcherEvent, mode: ExpansionTapMode, undoCaret: Int, swallowedText: String?) {
        guard insertion == nil else { resetMatcher(); return }
        listenOnlyInterrupted = false
        if mode == .active { held.begin() }
        generation += 1
        let token = generation
        deadline?.cancel()
        deadline = Task { [weak self] in
            do { try await Task.sleep(for: .seconds(1)) } catch { return }
            guard let self, self.generation == token else { return }
            self.insertion?.cancel()
            self.insertion = nil
            self.generation += 1
            self.resetMatcher()
            for event in self.held.timeout() { self.post(event, marker: EventTap.syntheticMarker) }
        }
        insertion = Task { [weak self] in
            guard let self else { return }
            defer {
                if self.generation == token {
                    self.insertion = nil
                    self.replayNext()
                }
            }
            do {
                // Listen-only cannot hold real input. Waiting lets the final key reach the app,
                // but continued typing can race deletion; there is no target acknowledgement.
                if mode == .listenOnly { try await Task.sleep(for: .milliseconds(30)) }
                try Task.checkCancellation()
                guard self.allowed, self.generation == token else { return }
                if mode == .listenOnly, self.listenOnlyInterrupted { self.resetMatcher(); return }
                switch action {
                case .expand(let match):
                    guard let snippet = self.snippets[match.snippetID] else { break }
                    let records = self.snippets
                    let clipboard = self.usesClipboard(snippet.content, visited: [])
                        ? NSPasteboard.general.string(forType: .string) : nil
                    let context = RenderContext(now: .now, calendar: .current, timeZone: .current,
                                                locale: .current, clipboard: clipboard,
                                                resolveSnippet: { records[$0]?.content })
                    // TODO(M3): form panel
                    let rendered = TemplateRenderer.render(snippet.content, context: context)
                    let text = CasePropagation.apply(match.caseStyle, to: rendered.text)
                    let offset = rendered.cursorOffset.map {
                        let prefix = String(decoding: Array(rendered.text.utf16.prefix($0)), as: UTF16.self)
                        return CasePropagation.apply(match.caseStyle, to: prefix).utf16.count
                    }
                    let plan = InsertionPlan(match: match, text: text, cursorOffset: offset, mode: mode,
                                             swallowedText: mode == .active ? swallowedText : nil)
                    try self.inserter.deleteBackward(count: plan.deleteCount)
                    let method: Inserter.Method = switch match.insertion {
                    case .automatic: .automatic
                    case .type: .type
                    case .paste: .paste
                    }
                    try await self.inserter.insert(plan.text, method: method)
                    try Task.checkCancellation()
                    guard self.generation == token, self.allowed else { return }
                    if let separator = plan.reemitSeparator { try self.inserter.type(separator) }
                    try self.inserter.moveCaretLeft(count: plan.caretLeftCount)
                    // UndoTracker adds the re-emitted separator itself.
                    self.matcher.didExpand(match: match, insertedCount: plan.text.count)
                    self.undoCaretLeftCount = plan.caretLeftCount
                    self.pendingUndo = (match, plan.text.count)
                    do {
                        try self.store.recordUsage(snippetID: match.snippetID,
                                                   charsSaved: max(0, text.count - match.typedTrigger.count))
                    } catch { Self.log.error("Usage recording failed: \(error)") }
                case .undo(let undo):
                    // Listen-only backspace already reached the document.
                    try self.inserter.moveCaretRight(count: undoCaret)
                    try self.inserter.deleteBackward(count: max(0, undo.deleteCount - (mode == .listenOnly ? 1 : 0)))
                    try self.inserter.type(undo.retype)
                }
            } catch {
                if !Task.isCancelled { Self.log.error("Insertion failed: \(error)") }
                if self.generation == token { self.resetMatcher() }
            }
        }
    }

    private func usesClipboard(_ source: String, visited: Set<UUID>) -> Bool {
        for node in TemplateParser.parse(source).nodes {
            guard case .tag(let tag) = node else { continue }
            switch tag.typed {
            case .clipboard: return true
            case .snippet(let id):
                if visited.count < 8, !visited.contains(id), let nested = snippets[id],
                   usesClipboard(nested.content, visited: visited.union([id])) { return true }
            default: break
            }
        }
        return false
    }

    private func scheduleReplay() {
        Task { [weak self] in self?.replayNext() }
    }

    private func replayNext() {
        guard insertion == nil else { return }
        guard let event = held.next() else {
            deadline?.cancel(); deadline = nil
            return
        }
        // Replay one event at a time. Its callback can start another expansion before
        // we release the remaining suffix, including triggers typed during a paste.
        post(event, marker: EventTap.replayMarker)
    }

    private func post(_ event: CGEvent, marker: Int64) {
        event.setIntegerValueField(.eventSourceUserData, value: marker)
        event.post(tap: .cgSessionEventTap)
    }

    /// Returns selected text using AX, falling back to clipboard-preserving Copy.
    func captureSelection() async -> String? {
        let system = AXUIElementCreateSystemWide()
        AXUIElementSetMessagingTimeout(system, 0.2)
        var focused: CFTypeRef?
        if AXUIElementCopyAttributeValue(system, kAXFocusedUIElementAttribute as CFString, &focused) == .success,
           let focused, CFGetTypeID(focused) == AXUIElementGetTypeID() {
            let element = unsafeDowncast(focused, to: AXUIElement.self)
            AXUIElementSetMessagingTimeout(element, 0.2)
            var value: CFTypeRef?
            if AXUIElementCopyAttributeValue(element, kAXSelectedTextAttribute as CFString, &value) == .success,
               let text = value as? String, !text.isEmpty { return text }
        }
        guard permissions.postEvent, !secureInputActive, !inserter.isBusy, insertion == nil else { return nil }
        invalidateContext()
        capturingSelection = true
        defer { capturingSelection = false }
        let board = NSPasteboard.general
        var saved: [NSPasteboardItem] = []
        for item in board.pasteboardItems ?? [] {
            let copy = NSPasteboardItem()
            for type in item.types {
                guard let data = item.data(forType: type), copy.setData(data, forType: type) else { return nil }
            }
            saved.append(copy)
        }
        let originalCount = board.changeCount
        let app = NSWorkspace.shared.frontmostApplication?.processIdentifier
        do { try inserter.copySelection() } catch { return nil }
        defer {
            if board.changeCount != originalCount {
                board.clearContents()
                if !saved.isEmpty { board.writeObjects(saved) }
            }
        }
        for _ in 0..<30 {
            do { try await Task.sleep(for: .milliseconds(10)) } catch { return nil }
            guard NSWorkspace.shared.frontmostApplication?.processIdentifier == app else { return nil }
            if board.changeCount != originalCount {
                let text = board.string(forType: .string)
                return text?.isEmpty == false ? text : nil
            }
        }
        return nil
    }
}
