import AppKit
import Foundation
import KudzuCore
import KudzuDesign

/// Convenience actions for Settings, Insights, Onboarding and the Permission companion.
extension AppModel {
    /// The active section when `view` is `.settings`, defaulting to `.general`.
    public var currentSettingsSection: SettingsSection {
        if case let .settings(section) = view {
            return section ?? .general
        }
        return .general
    }

    /// Switches the Settings view to `section`.
    public func setSettingsSection(_ section: SettingsSection) {
        setView(.settings(section: section))
    }

    /// Moves to the previous (`-1`) or next (`+1`) Settings section.
    public func stepSettingsSection(_ step: Int) {
        setSettingsSection(SettingsNav.neighbour(of: currentSettingsSection, step: step))
    }

    /// Applies a partial settings update, ignoring transient errors after optimistic state update.
    public func patchSettings(_ build: (inout SettingsPatch) -> Void) async {
        try? await updateSettings(SettingsPatch(build))
    }

    /// Reloads library, settings, status, usage, and conflicts from the backend.
    public func refreshState() async {
        guard let state = try? await backend.getState() else { return }
        library = state.library
        settings = state.settings
        status = state.status
        usage = Stats.usageById(state.library)
        refreshConflicts()
    }

    /// Applies a theme change using `ThemeSwitch` so colour transitions do not wash out mid-flip.
    public func selectTheme(_ theme: Theme) async {
        let appearance: NSAppearance? = switch theme {
        case .dark: NSAppearance(named: .darkAqua)
        case .light: NSAppearance(named: .aqua)
        case .system: nil
        }
        ThemeSwitch.shared.apply(appearance)
        try? await updateSettings(SettingsPatch { $0.theme = theme })
    }

    // MARK: Hotkeys

    /// Registers a new palette hotkey and surfaces a toast on success or failure.
    public func applyPaletteHotkey(_ accelerator: String, toastCenter: ToastCenter = .shared) async -> Bool {
        do {
            settings = try await backend.setPaletteHotkey(accelerator: accelerator)
            toastCenter.show("Shortcut updated", kind: .success)
            return true
        } catch {
            toastCenter.show("That shortcut is taken", kind: .error, description: Self.errorMessage(error))
            return false
        }
    }

    /// Registers a new clipboard picker hotkey and surfaces a toast on success or failure.
    public func applyClipboardHotkey(_ accelerator: String, toastCenter: ToastCenter = .shared) async -> Bool {
        do {
            settings = try await backend.setClipboardHotkey(accelerator: accelerator)
            toastCenter.show("Shortcut updated", kind: .success)
            return true
        } catch {
            toastCenter.show("That shortcut is taken", kind: .error, description: Self.errorMessage(error))
            return false
        }
    }

    // MARK: iCloud Sync

    /// Queries the current iCloud sync status.
    public func loadSyncStatus() async -> SyncStatus {
        (try? await backend.syncStatus()) ?? .off
    }

    /// Requests an immediate iCloud sync pass.
    public func triggerSyncNow(toastCenter: ToastCenter = .shared) async {
        do {
            try await backend.syncNow()
        } catch {
            toastCenter.show(Self.errorMessage(error), kind: .error)
        }
    }

    // MARK: AI Agents

    /// Queries the MCP agent setup details.
    public func loadAgentSetup() async -> AgentSetup? {
        try? await backend.agentSetup()
    }

    /// Installs or updates the bundled Claude skill file.
    @discardableResult
    public func installSkill(toastCenter: ToastCenter = .shared) async -> AgentSetup? {
        do {
            let path = try await backend.installAgentSkill()
            toastCenter.show("Skill installed", kind: .success, description: path)
            return await loadAgentSetup()
        } catch {
            toastCenter.show("Couldn't install the skill", kind: .error, description: Self.errorMessage(error))
            return nil
        }
    }

    // MARK: Data & Backups

    /// Loads stored backups and detected Espanso configurations concurrently.
    public func loadDataSources() async -> (backups: [BackupInfo], espanso: [EspansoDetection]) {
        async let backups = (try? backend.listBackups()) ?? []
        async let espanso = (try? backend.detectEspanso()) ?? []
        return await (backups, espanso)
    }

    /// Creates a manual backup snapshot and returns the updated backup list.
    public func createManualBackup(toastCenter: ToastCenter = .shared) async -> [BackupInfo]? {
        do {
            _ = try await backend.backupNow()
            let list = (try? await backend.listBackups()) ?? []
            toastCenter.show("Backup saved", kind: .success)
            return list
        } catch {
            toastCenter.show(Self.errorMessage(error), kind: .error)
            return nil
        }
    }

    /// Restores the library from `backup` and refreshes model state.
    public func restoreFromBackup(_ backup: BackupInfo, dateLabel: String, toastCenter: ToastCenter = .shared) async -> [BackupInfo]? {
        // The sheet already showed `dateLabel`. The toast matches the web copy, which does not repeat the date.
        _ = dateLabel
        do {
            try await backend.restoreBackup(path: backup.path)
            await refreshState()
            let list = (try? await backend.listBackups()) ?? []
            toastCenter.show("Library restored", kind: .success, description: "Your previous library was backed up first.")
            return list
        } catch {
            toastCenter.show(Self.errorMessage(error), kind: .error)
            return nil
        }
    }

    /// Imports an Espanso directory at `path`, refreshes the library, and returns the summary.
    public func importDetectedEspanso(path: String) async throws -> ImportSummary {
        let summary = try await backend.importEspanso(path: path)
        await refreshState()
        return summary
    }

    /// Reveals the application data directory in Finder.
    public func revealDataDirectory() async {
        try? await backend.revealDataFolder()
    }

    // MARK: About & Updates

    /// Loads the app version and current update state.
    public func loadAboutInfo() async -> (version: String, updateState: UpdateState) {
        async let version = (try? backend.appVersion()) ?? "0.1.0"
        async let state = (try? backend.updateState()) ?? .idle
        return await (version, state)
    }

    /// Runs an explicit update check and returns the resulting `UpdateState`.
    public func triggerUpdateCheck() async -> UpdateState {
        do {
            return try await backend.checkForUpdates()
        } catch {
            return .error(message: Self.errorMessage(error), needsMove: false)
        }
    }

    /// Installs a downloaded update and restarts, or returns an `.error` state if it fails.
    public func triggerRestartToUpdate() async -> UpdateState? {
        do {
            try await backend.restartToUpdate()
            return nil
        } catch {
            return .error(message: Self.errorMessage(error), needsMove: false)
        }
    }

    // MARK: Permissions & Onboarding

    /// Opens the permission companion window for the first missing permission, falling back to System Settings.
    public func openPermissionSetup() async {
        let kind: PermissionKind = status.permissions.accessibility ? .inputMonitoring : .accessibility
        do {
            try await backend.showPermissions(kind: kind)
        } catch {
            let pane: SystemSettingsPane = kind == .accessibility ? .accessibility : .inputMonitoring
            try? await backend.openSystemSettings(pane: pane)
        }
    }

    /// Finishes onboarding, optionally pausing expansion if the user skipped permissions.
    public func completeOnboarding(startPaused: Bool = false) async {
        if startPaused {
            try? await setPaused(true)
        }
        try? await updateSettings(SettingsPatch { $0.onboardingCompleted = true })
    }

    // MARK: Error Formatting

    static func errorMessage(_ error: any Error) -> String {
        if let localized = (error as? LocalizedError)?.errorDescription, !localized.isEmpty {
            return localized
        }
        return String(describing: error)
    }
}
