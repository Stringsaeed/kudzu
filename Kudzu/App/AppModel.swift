import KudzuCore
import Observation
import SwiftUI

/// Composition root shared by every scene. Owns the store, permissions and expansion engine.
@MainActor @Observable
final class AppModel {
    static let onboardingCompletedDefaultsKey = "hasCompletedOnboarding"

    let store: KudzuStore
    let permissions: Permissions
    let engine: ExpansionEngine
    let backupStore: BackupStore
    let backupScheduler: BackupScheduler

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
        let backupStore = BackupStore(directory: BackupScheduler.defaultBackupDirectory())
        let backupScheduler = BackupScheduler(store: store, backupStore: backupStore)

        self.store = store
        self.permissions = permissions
        self.engine = ExpansionEngine(store: store, permissions: permissions)
        self.backupStore = backupStore
        self.backupScheduler = backupScheduler

        engine.start()
        backupScheduler.start()
    }

    /// Whether the user has finished (or skipped) the welcome flow.
    static var hasCompletedOnboarding: Bool {
        UserDefaults.standard.bool(forKey: onboardingCompletedDefaultsKey)
    }

    @ObservationIgnored private var didCheckOnboarding = false

    /// Opens the welcome window once per launch until onboarding is complete.
    func openOnboardingIfNeeded(_ openWindow: OpenWindowAction) {
        guard !didCheckOnboarding else { return }
        didCheckOnboarding = true
        if !Self.hasCompletedOnboarding { openWindow(id: SceneID.onboarding) }
    }
}

/// Window identifiers used with `openWindow(id:)`.
enum SceneID {
    static let main = "main"
    static let onboarding = "onboarding"
}
