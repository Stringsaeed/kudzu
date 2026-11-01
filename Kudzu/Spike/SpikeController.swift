import AppKit
import Carbon
import Observation

@MainActor @Observable
final class SpikeController {
    let permissions = Permissions()
    let secureInput = SecureInputMonitor()
    let frontmost = FrontmostApp()
    let eventTap = EventTap()
    var paused = false { didSet { invalidateContext() } }
    private(set) var lastResult = "Ready for manual tests"
    private(set) var hotKeyStatus: Int32 = 0
    private let matcher = SpikeMatcher()
    private let inserter = Inserter()
    private let panel = FloatingPanel()
    private var hotKey: GlobalHotKey?
    private var poll: Task<Void, Never>?
    private var expansion: Task<Void, Never>?
    private var expanding = false
    private var stopped = false
    private var contextVersion = 0
    private var terminationObserver: NSObjectProtocol?

    init() {
        eventTap.onSignal = { [weak self] in self?.receive($0) }
        frontmost.onActivation = { [weak self] in
            self?.invalidateContext()
            self?.panel.hide()
        }
        terminationObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.willTerminateNotification, object: nil, queue: .main
        ) { [weak self] _ in MainActor.assumeIsolated { self?.stop() } }
        hotKey = GlobalHotKey { [weak self] in self?.showPanel() }
        hotKeyStatus = hotKey?.status ?? -1
        poll = Task { [weak self] in
            while !Task.isCancelled {
                self?.refresh()
                try? await Task.sleep(for: .seconds(1))
            }
        }
    }

    isolated deinit { stop() }

    func refresh() {
        permissions.refresh()
        secureInput.refresh()
        if secureInput.isEnabled { matcher.reset() }
        if permissions.inputMonitoring { eventTap.start() }
        else { eventTap.stop() }
    }

    func showPanel() {
        matcher.reset()
        panel.show()
    }

    func stop() {
        guard !stopped else { return }
        stopped = true
        if let terminationObserver { NotificationCenter.default.removeObserver(terminationObserver) }
        terminationObserver = nil
        poll?.cancel()
        expansion?.cancel()
        inserter.restorePasteboard()
        eventTap.stop()
        frontmost.stop()
        hotKey?.stop()
        panel.hide()
    }

    private func invalidateContext() {
        contextVersion += 1
        matcher.reset()
    }

    private func receive(_ signal: EventTap.Signal) {
        contextVersion += 1
        guard !paused, !expanding, !secureInput.isEnabled else { matcher.reset(); return }
        let excluded = ["com.1password.1password", "com.agilebits.onepassword7", "com.bitwarden.desktop",
                        "com.apple.keychainaccess", "com.apple.loginwindow"]
        guard let app = NSWorkspace.shared.frontmostApplication, app.processIdentifier != ProcessInfo.processInfo.processIdentifier,
              !excluded.contains(app.bundleIdentifier ?? "") else { matcher.reset(); return }
        guard let match = matcher.consume(signal) else { return }
        guard permissions.postEvent else { lastResult = "Grant Post Event before expansion"; return }
        expanding = true
        let version = contextVersion
        expansion = Task { [weak self] in
            guard let self else { return }
            defer { self.expanding = false; self.matcher.reset() }
            // A listen-only tap cannot consume the final trigger key. Let the target process it first.
            do {
                try await Task.sleep(for: .milliseconds(30))
                guard !self.stopped, !self.paused, self.contextVersion == version, !IsSecureEventInputEnabled(),
                      NSWorkspace.shared.frontmostApplication?.processIdentifier == app.processIdentifier else { return }
                try self.inserter.deleteBackward(count: match.trigger.count)
                try await self.inserter.insert(match.text, method: match.method)
                self.lastResult = "Posted \(match.trigger) (verify in target app)"
            } catch { self.lastResult = "Insertion failed: \(error)" }
        }
    }
}
