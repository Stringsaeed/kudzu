import KudzuUI
import SwiftUI

/// The sections of settings mode, in sidebar order. Settings is a mode of the main window, not a separate window.
enum SettingsSection: String, CaseIterable, Identifiable, Hashable {
    case general, apps, sync, backups, importExport, about

    var id: String { rawValue }

    /// Sentence-case title, as the design system writes it.
    var title: String {
        switch self {
        case .general: "General"
        case .apps: "Apps"
        case .sync: "Sync"
        case .backups: "Backups"
        case .importExport: "Import & export"
        case .about: "About"
        }
    }

    /// Kudzu icon for the section list.
    var icon: KzIconName {
        switch self {
        case .general: .sliders
        case .apps: .app
        case .sync: .cloud
        case .backups: .history
        case .importExport: .sort
        case .about: .info
        }
    }
}

/// The detail-pane page for a settings section.
struct SettingsPage: View {
    let section: SettingsSection

    var body: some View {
        switch section {
        case .general: GeneralSettingsTab()
        case .apps: AppsSettingsTab()
        case .sync: SyncSettingsTab()
        case .backups: BackupsSettingsTab()
        case .importExport: ImportExportSettingsTab()
        case .about: AboutSettingsTab()
        }
    }
}

/// What the main window shows.
enum MainWindowMode: Hashable {
    case library
    case settings(SettingsSection)
}

/// Work the main window should do when a menu command arrives while it is closed or not key.
enum MainWindowRequest: Equatable {
    case newSnippet
    case focusSearch
    case focusSidebar
    case focusResults
    case focusEditor
    case clearSearch
}
