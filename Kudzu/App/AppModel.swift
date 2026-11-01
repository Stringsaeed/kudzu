import KudzuCore
import Observation
import SwiftUI

/// Composition root shared by every scene. Owns the store, permissions, expansion engine,
/// search palette and its global shortcut.
@MainActor @Observable
final class AppModel {
    static let onboardingCompletedDefaultsKey = "hasCompletedOnboarding"

    let store: KudzuStore
    let permissions: Permissions
    let engine: ExpansionEngine
    let backupStore: BackupStore
    let backupScheduler: BackupScheduler
    let palette: SearchPaletteController

    /// Set when the store could not be opened; the UI shows it instead of crashing.
    private(set) var storeError: String?

    /// Text captured by "New Snippet from Selection". The main window consumes it
    /// (creates a snippet with this content, selects it, then sets this back to nil).
    var pendingNewSnippetContent: String?

    /// Library or settings. ⌘, and the menu bar's Settings… switch the main window into settings mode.
    var mainWindowMode: MainWindowMode = .library

    /// Switches the main window to settings; callers open and activate the window themselves.
    func showSettings(_ section: SettingsSection = .general) {
        mainWindowMode = .settings(section)
    }

    /// Snippet the palette's ⌘E asked to edit. The main window consumes it (selects the snippet,
    /// clearing a search or scope that would hide it, then sets this back to nil).
    var pendingSelectSnippetID: UUID?

    /// The search palette's global shortcut.
    private(set) var paletteShortcut: HotKeyShortcut
    /// Why the palette shortcut isn't registered, or nil when it is.
    private(set) var paletteShortcutError: String?
    @ObservationIgnored private var paletteHotKey: GlobalHotKey?
    @ObservationIgnored private var openWindowAction: OpenWindowAction?

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
        self.palette = SearchPaletteController(store: store)
        self.paletteShortcut = HotKeyShortcut.savedPaletteShortcut()

        engine.start()
        backupScheduler.start()
        startPalette()
    }

    // MARK: - Search palette

    private func startPalette() {
        palette.onInsert = { [weak self] id in await self?.engine.insertSnippet(id: id) }
        palette.onEdit = { [weak self] id in self?.revealSnippetInEditor(id) }
        paletteHotKey = GlobalHotKey { [weak self] in self?.togglePalette() }
        registerPaletteShortcut(paletteShortcut)
    }

    /// Opens the palette, or closes it if it is already open.
    func togglePalette() {
        palette.toggle()
    }

    /// Opens the palette.
    func showPalette() {
        palette.show()
    }

    /// Changes the palette shortcut. When it can't be registered (e.g. another app owns it) the
    /// previous shortcut stays active, the error is published, and this returns false.
    @discardableResult
    func setPaletteShortcut(_ shortcut: HotKeyShortcut) -> Bool {
        let previous = paletteShortcut
        guard registerPaletteShortcut(shortcut) else {
            if previous != shortcut, let paletteHotKey { try? paletteHotKey.register(previous) }
            return false
        }
        paletteShortcut = shortcut
        HotKeyShortcut.savePaletteShortcut(shortcut)
        return true
    }

    /// Restores ⌃⌥Space.
    func resetPaletteShortcut() {
        setPaletteShortcut(.paletteDefault)
    }

    /// Pauses the global shortcut while a recorder captures a new one, so pressing the current
    /// shortcut records it instead of opening the palette.
    func setPaletteShortcutSuspended(_ suspended: Bool) {
        if suspended {
            paletteHotKey?.unregister()
        } else if paletteHotKey?.shortcut == nil {
            registerPaletteShortcut(paletteShortcut)
        }
    }

    @discardableResult
    private func registerPaletteShortcut(_ shortcut: HotKeyShortcut) -> Bool {
        guard let paletteHotKey else { return false }
        do {
            try paletteHotKey.register(shortcut)
            paletteShortcutError = nil
            return true
        } catch {
            paletteShortcutError = "\(shortcut.displayString): \(error.message)"
            return false
        }
    }

    // MARK: - Windows

    /// Keeps SwiftUI's window opener so non-view code (the palette) can open scenes.
    func attach(openWindow: OpenWindowAction) {
        openWindowAction = openWindow
    }

    /// Opens the main window with the snippet selected.
    func revealSnippetInEditor(_ id: UUID) {
        pendingSelectSnippetID = id
        openWindowAction?(id: SceneID.main)
        NSApplication.shared.activate()
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
