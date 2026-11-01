import KudzuCore
import Observation
import SwiftUI

/// Composition root shared by every scene. Owns the store, permissions and expansion engine.
@MainActor @Observable
final class AppModel {
    let store: KudzuStore
    let permissions: Permissions
    let engine: ExpansionEngine

    /// Set when the store could not be opened; the UI shows it instead of crashing.
    private(set) var storeError: String?

    /// Text captured by "New Snippet from Selection". The main window consumes it
    /// (creates a snippet with this content, selects it, then sets this back to nil).
    var pendingNewSnippetContent: String?

    init() {
        let permissions = Permissions()
        let store: KudzuStore
        do {
            // iCloud sync needs the CloudKit entitlement, which lands in M4.
            store = try KudzuStore(syncEnabled: false)
        } catch {
            storeError = "Couldn't open the snippet library: \(error.localizedDescription)"
            store = try! KudzuStore.inMemory()
        }
        self.store = store
        self.permissions = permissions
        self.engine = ExpansionEngine(store: store, permissions: permissions)
    }
}

/// Window identifiers used with `openWindow(id:)`.
enum SceneID {
    static let main = "main"
    static let onboarding = "onboarding"
}
