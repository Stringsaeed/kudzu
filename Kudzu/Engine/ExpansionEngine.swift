import KudzuCore
import Observation

// Placeholder: owned by the engine task. Keep the init and public surface stable;
// the menu bar and settings read it.

/// Watches typing system-wide and expands snippets from the store.
@MainActor @Observable
final class ExpansionEngine {
    /// When true, no expansions happen (menu bar "Pause").
    var isPaused = false
    /// True while macOS Secure Input blocks event monitoring.
    private(set) var secureInputActive = false
    /// App that enabled Secure Input, when known.
    private(set) var secureInputOwner: String?
    /// One-line status for the menu bar, e.g. "Listening" or "Needs Input Monitoring".
    private(set) var statusText = "Not started"

    private let store: KudzuStore
    private let permissions: Permissions

    init(store: KudzuStore, permissions: Permissions) {
        self.store = store
        self.permissions = permissions
    }

    /// Starts listening once permissions allow it. Safe to call repeatedly.
    func start() {}

    /// Stops listening and restores any pending clipboard.
    func stop() {}

    /// Returns the text currently selected in the frontmost app, for "New Snippet from Selection".
    func captureSelection() async -> String? { nil }
}
