import AppKit
import KudzuCore
import KudzuUI
import ServiceManagement
import SwiftUI
import UniformTypeIdentifiers

/// Compatibility entry point until the main-window settings mode replaces the Settings scene.
struct SettingsView: View {
    var body: some View { SettingsPage(section: .general) }
}

/// Shared settings layout. Pages own their scrolling; hosts should not wrap them in a ScrollView.
struct SettingsPageLayout<Content: View>: View {
    let title: String
    var description: String? = nil
    @ViewBuilder var content: Content

    @Environment(\.kzSnapshotRendering) private var snapshotRendering
    @Environment(\.settingsModalPresentation) private var inModal

    var body: some View {
        Group {
            if snapshotRendering {
                pageContent.frame(maxHeight: .infinity, alignment: .top)
            } else {
                ScrollView { pageContent }
            }
        }
        .background(WorkspaceTheme.paper)
    }

    private var pageContent: some View {
        VStack(alignment: .leading, spacing: WorkspaceSurface.sectionGap) {
            if !inModal {
                VStack(alignment: .leading, spacing: 6) {
                    Text(title)
                        .font(WorkspaceTheme.title)
                        .foregroundStyle(WorkspaceTheme.ink)
                        .accessibilityAddTraits(.isHeader)
                    if let description {
                        Text(description)
                            .font(WorkspaceTheme.body)
                            .foregroundStyle(WorkspaceTheme.secondary)
                    }
                }
            } else if let description {
                Text(description)
                    .font(WorkspaceTheme.body)
                    .foregroundStyle(WorkspaceTheme.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            content
        }
        .frame(maxWidth: inModal ? WorkspaceSurface.contentMaxWidth : .infinity, alignment: .leading)
        .frame(maxWidth: .infinity)
        .padding(.horizontal, WorkspaceSurface.pagePadding)
        .padding(.top, inModal ? WorkspaceSurface.pagePadding : 32)
        .padding(.bottom, WorkspaceSurface.pagePadding)
    }
}

struct SettingsMessage: View {
    var text: String
    var isError = false
    var body: some View {
        WorkspaceInlineMessage(text: text, isError: isError)
    }
}

// MARK: - General Tab

struct GeneralSettingsTab: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openWindow) private var openWindow

    @State private var launchAtLogin = false
    @State private var launchAtLoginRequiresApproval = false
    @State private var launchAtLoginError: String?
    @State private var typingSpeedWPM = 40
    @State private var defaultTriggerPrefix = ";"
    @State private var settingsError: String?

    var body: some View {
        SettingsPageLayout(title: "General") {
            WorkspaceSection("Startup") {
                WorkspaceSettingRow("Launch Kudzu at login") {
                    Toggle("Launch Kudzu at login", isOn: Binding(
                        get: { launchAtLogin }, set: { setLaunchAtLogin($0) }
                    ))
                    .labelsHidden()
                }
                if launchAtLoginRequiresApproval {
                    WorkspaceInlineMessage(text: "Requires approval in System Settings → General → Login Items.")
                        .padding(.vertical, 8)
                }
                if let launchAtLoginError {
                    SettingsMessage(text: launchAtLoginError, isError: true)
                }
            }
            WorkspaceSection("Snippets & stats") {
                WorkspaceSettingRow("Typing speed", description: "Used to estimate time saved") {
                    Stepper("\(typingSpeedWPM) wpm", value: Binding(
                        get: { typingSpeedWPM }, set: { updateTypingSpeed($0) }
                    ), in: 10...240, step: 5)
                    .monospacedDigit()
                    .accessibilityLabel("Typing speed")
                    .accessibilityValue("\(typingSpeedWPM) words per minute")
                }
                WorkspaceSettingRow("Default trigger prefix", description: "Added to new triggers you type") {
                    TextField("Prefix", text: Binding(
                        get: { defaultTriggerPrefix }, set: { updateDefaultTriggerPrefix($0) }
                    ))
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 72)
                    .font(WorkspaceTheme.mono)
                    .accessibilityLabel("Default trigger prefix")
                }
                WorkspaceSettingRow("Search palette shortcut") { PaletteShortcutSetting() }
            }
            WorkspaceSection("Onboarding") {
                WorkspaceSettingRow("Setup assistant") {
                    Button("Show welcome again") {
                        openWindow(id: SceneID.onboarding)
                        NSApplication.shared.activate()
                    }
                    .controlSize(.small)
                }
            }
            if let settingsError { SettingsMessage(text: settingsError, isError: true) }
        }
        .onAppear {
            loadSettings()
        }
    }

    private func loadSettings() {
        refreshLaunchAtLoginStatus()
        do {
            let synced = try model.store.syncedSettings()
            typingSpeedWPM = synced.typingSpeedWPM
            defaultTriggerPrefix = synced.defaultTriggerPrefix
            settingsError = nil
        } catch {
            settingsError = error.localizedDescription
        }
    }

    private func refreshLaunchAtLoginStatus() {
        let status = SMAppService.mainApp.status
        launchAtLogin = (status == .enabled || status == .requiresApproval)
        launchAtLoginRequiresApproval = (status == .requiresApproval)
    }

    private func setLaunchAtLogin(_ enabled: Bool) {
        launchAtLoginError = nil
        do {
            if enabled {
                try SMAppService.mainApp.register()
            } else {
                try SMAppService.mainApp.unregister()
            }
        } catch {
            launchAtLoginError = "Couldn't update Login Item: \(error.localizedDescription)"
        }
        refreshLaunchAtLoginStatus()
    }

    private func updateTypingSpeed(_ newValue: Int) {
        typingSpeedWPM = newValue
        do {
            try model.store.updateSyncedSettings { settings in
                settings.typingSpeedWPM = newValue
            }
            settingsError = nil
        } catch {
            settingsError = error.localizedDescription
        }
    }

    private func updateDefaultTriggerPrefix(_ newValue: String) {
        defaultTriggerPrefix = newValue
        do {
            try model.store.updateSyncedSettings { settings in
                settings.defaultTriggerPrefix = newValue
            }
            settingsError = nil
        } catch {
            settingsError = error.localizedDescription
        }
    }
}

// MARK: - Sync Tab

struct SyncSettingsTab: View {
    @Environment(AppModel.self) private var model
    @State private var deviceID = ""

    private var phase: SyncPhase { model.syncMonitor.snapshot.phase }

    var body: some View {
        SettingsPageLayout(title: "Sync", description: "Keep your library the same on every Mac signed in to your iCloud account.") {
            WorkspaceSection("iCloud") {
                WorkspaceSettingRow("iCloud sync", description: phase.statusText) {
                    Toggle("iCloud sync", isOn: Binding(
                        get: {
                            model.syncMonitor.snapshot.blocked == nil
                                && model.syncMonitor.snapshot.preferenceEnabled
                        },
                        set: { model.setCloudSyncEnabled($0) }
                    ))
                    .labelsHidden()
                    .disabled(model.syncMonitor.snapshot.blocked != nil)
                }
                if case .upToDate(let date?) = phase {
                    WorkspaceSettingRow("Last synced") {
                        Text(date.formatted(date: .abbreviated, time: .shortened))
                            .font(WorkspaceTheme.mono)
                            .foregroundStyle(WorkspaceTheme.secondary)
                    }
                }
                if case .needsRestart = phase {
                    WorkspaceSettingRow("Apply the change", description: "The library that's open keeps its current sync mode.") {
                        Button("Quit and reopen") { NSApp.terminate(nil) }
                            .controlSize(.small)
                    }
                }
            }
            WorkspaceSection("This Mac") {
                WorkspaceSettingRow("Device ID", description: "Usage is counted separately on each Mac, then added together.") {
                    Text(deviceID.isEmpty ? "—" : deviceID)
                        .font(WorkspaceTheme.mono)
                        .foregroundStyle(WorkspaceTheme.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .textSelection(.enabled)
                        .frame(maxWidth: 200, alignment: .trailing)
                }
            }
        }
        .onAppear {
            if let local = try? model.store.localSettings() {
                deviceID = local.deviceID
            }
        }
    }
}

// MARK: - Backups Tab

struct BackupsSettingsTab: View {
    struct BackupRow: Identifiable, Hashable {
        var info: BackupInfo
        var snippetCount: Int?
        var id: URL { info.url }
    }

    @Environment(AppModel.self) private var model

    @Environment(\.kzSnapshotRendering) private var snapshotRendering
    @State private var rows: [BackupRow] = []

    init(snapshotRows: [BackupRow] = []) {
        _rows = State(initialValue: snapshotRows)
        _selectedBackupURL = State(initialValue: snapshotRows.first?.info.url)
    }
    @State private var selectedBackupURL: URL?
    @State private var backupPendingRestore: BackupRow?
    @State private var statusMessage: String?
    @State private var errorMessage: String?

    private var selectedRow: BackupRow? {
        rows.first { $0.info.url == selectedBackupURL }
    }

    var body: some View {
        SettingsPageLayout(title: "Backups", description: "Daily snapshots of your library, plus one before every import.") {
            WorkspaceSection("Snapshots", description: "Keeps the last 14 snapshots.") {
                WorkspaceSectionAction(title: "Back up now", action: createManualBackup)
                    .padding(.bottom, 4)
                if rows.isEmpty {
                    WorkspaceSettingRow("No backups yet", description: "Choose Back up now to save your first snapshot.") {
                        EmptyView()
                    }
                } else {
                    if snapshotRendering {
                        VStack(spacing: 0) { backupRows }
                    } else {
                        List(selection: $selectedBackupURL) { backupRows }
                            .listStyle(.plain)
                            .scrollContentBackground(.hidden)
                            .frame(height: min(360, CGFloat(rows.count) * 56 + 8))
                    }
                }
            }
            HStack(spacing: 10) {
                Button("Restore…") { backupPendingRestore = selectedRow }
                    .controlSize(.small)
                    .disabled(selectedRow == nil)
                Button("Reveal in Finder") { revealInFinder() }
                    .controlSize(.small)
                    .buttonStyle(.borderless)
            }
            if let statusMessage { SettingsMessage(text: statusMessage) }
            if let errorMessage { SettingsMessage(text: errorMessage, isError: true) }
        }
        .onAppear {
            if !snapshotRendering { reloadBackups() }
        }
        .alert(
            "Restore backup?",
            isPresented: Binding(
                get: { backupPendingRestore != nil },
                set: { if !$0 { backupPendingRestore = nil } }
            ),
            presenting: backupPendingRestore
        ) { target in
            Button("Cancel", role: .cancel) {
                backupPendingRestore = nil
            }
            Button("Restore", role: .destructive) {
                performRestore(target.info)
            }
        } message: { target in
            Text("Restoring “\(target.info.filename)” from \(target.info.createdAt.formatted(date: .abbreviated, time: .shortened)) will replace your current snippet library. A manual safety backup of your current library will be saved first.")
        }
    }

    private var backupRows: some View {
        ForEach(rows) { row in
            HStack(alignment: .center, spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(row.info.createdAt.formatted(date: .abbreviated, time: .standard))
                        .font(WorkspaceTheme.body)
                        .foregroundStyle(WorkspaceTheme.ink)
                    Text(row.info.filename)
                        .font(WorkspaceTheme.caption)
                        .foregroundStyle(WorkspaceTheme.secondary)
                        .lineLimit(1)
                }
                Spacer(minLength: 8)
                Text(reasonLabel(row.info.reason))
                    .font(WorkspaceTheme.caption)
                    .foregroundStyle(row.info.reason == .preImport ? KzColor.warning : WorkspaceTheme.secondary)
                if let count = row.snippetCount {
                    Text("\(count) snippet\(count == 1 ? "" : "s")")
                        .font(WorkspaceTheme.caption)
                        .foregroundStyle(WorkspaceTheme.secondary)
                        .frame(width: 72, alignment: .trailing)
                }
            }
            .padding(.vertical, 8)
            .tag(row.info.url)
        }
    }

    private func reasonLabel(_ reason: BackupReason) -> String {
        switch reason {
        case .daily: "Daily"
        case .preImport: "Pre-import"
        case .manual: "Manual"
        }
    }

    private func reloadBackups() {
        do {
            let infos = try model.backupStore.list()
            rows = infos.map { info in
                let count = (try? model.backupStore.load(info))?.snippets.count
                return BackupRow(info: info, snippetCount: count)
            }
            if selectedBackupURL == nil {
                selectedBackupURL = rows.first?.info.url
            }
            errorMessage = nil
        } catch {
            errorMessage = "Couldn't list backups: \(error.localizedDescription)"
        }
    }

    private func createManualBackup() {
        statusMessage = nil
        errorMessage = nil
        do {
            let library = try model.libraryForTransfer()
            let url = try model.backupStore.write(library, reason: .manual)
            reloadBackups()
            selectedBackupURL = url
            statusMessage = "Saved \(url.lastPathComponent)"
        } catch {
            errorMessage = "Backup failed: \(error.localizedDescription)"
        }
    }

    private func performRestore(_ info: BackupInfo) {
        statusMessage = nil
        errorMessage = nil
        do {
            _ = try model.libraryForTransfer()
            let safetyURL = try ImportCoordinator.restoreBackup(
                info,
                into: model.store,
                backupStore: model.backupStore
            )
            reloadBackups()
            statusMessage = "Restored \(info.filename) (safety backup: \(safetyURL.lastPathComponent))"
        } catch {
            errorMessage = "Restore failed: \(error.localizedDescription)"
        }
    }

    private func revealInFinder() {
        let dir = model.backupStore.directory
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        if let selected = selectedRow {
            NSWorkspace.shared.activateFileViewerSelecting([selected.info.url])
        } else {
            NSWorkspace.shared.selectFile(nil, inFileViewerRootedAtPath: dir.path)
        }
    }
}

// MARK: - Import & Export Tab

struct ImportExportSettingsTab: View {
    @Environment(AppModel.self) private var model

    @State private var showingEspansoSheet = false
    @State private var lastImportSummary: ImportSummary?
    @State private var statusMessage: String?
    @State private var errorMessage: String?

    var body: some View {
        SettingsPageLayout(title: "Import & export", description: "Move snippets in from Espanso, or take your library anywhere.") {
            WorkspaceSection("Import", description: "A backup is taken before every import.") {
                WorkspaceSettingRow("Espanso YAML", description: "match/*.yml files") {
                    Button("Import…") { showingEspansoSheet = true }
                        .controlSize(.small)
                }
                WorkspaceSettingRow("Kudzu JSON", description: "A library exported from Kudzu") {
                    Button("Import…") { importKudzuLibrary() }
                        .controlSize(.small)
                }
            }
            WorkspaceSection("Export") {
                WorkspaceSettingRow("Kudzu JSON", description: "Lossless: rules, stats and groups included") {
                    Button("Export…") { exportKudzuLibrary() }
                        .controlSize(.small)
                }
                WorkspaceSettingRow("Espanso YAML", description: "Triggers and content only") {
                    Button("Export…") { exportToEspanso() }
                        .controlSize(.small)
                }
            }
            if let summary = lastImportSummary {
                WorkspaceSection("Latest import result") {
                    ImportSummaryCard(summary: summary)
                }
            }
            if let statusMessage { SettingsMessage(text: statusMessage) }
            if let errorMessage { SettingsMessage(text: errorMessage, isError: true) }
        }
        .sheet(isPresented: $showingEspansoSheet) {
            VStack(alignment: .leading, spacing: 16) {
                HStack {
                    Text("Import from Espanso")
                        .font(WorkspaceTheme.title)
                    Spacer()
                    Button("Done") { showingEspansoSheet = false }
                        .keyboardShortcut(.cancelAction)
                }
                WorkspaceHairline()
                ScrollView {
                    EspansoImportView()
                        .environment(model)
                }
            }
            .padding(WorkspaceSurface.pagePadding)
            .frame(width: 540, height: 380)
            .background(WorkspaceTheme.paper)
        }
    }

    private func importKudzuLibrary() {
        statusMessage = nil
        errorMessage = nil

        let panel = NSOpenPanel()
        panel.title = "Import Kudzu library"
        panel.allowedContentTypes = [.json]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false

        guard panel.runModal() == .OK, let url = panel.url else { return }

        let accessed = url.startAccessingSecurityScopedResource()
        defer {
            if accessed {
                url.stopAccessingSecurityScopedResource()
            }
        }

        do {
            _ = try model.libraryForTransfer()
            lastImportSummary = try ImportCoordinator.importKudzuJSON(
                from: url,
                into: model.store,
                backupStore: model.backupStore
            )
            statusMessage = "Imported Kudzu library from \(url.lastPathComponent)."
        } catch {
            errorMessage = "Import failed: \(error.localizedDescription)"
        }
    }

    private func exportKudzuLibrary() {
        statusMessage = nil
        errorMessage = nil

        let panel = NSSavePanel()
        panel.title = "Export Kudzu library"
        panel.allowedContentTypes = [.json]
        panel.nameFieldStringValue = "kudzu-library.json"
        panel.canCreateDirectories = true

        guard panel.runModal() == .OK, let url = panel.url else { return }

        let accessed = url.startAccessingSecurityScopedResource()
        defer {
            if accessed {
                url.stopAccessingSecurityScopedResource()
            }
        }

        do {
            let library = try model.libraryForTransfer()
            let data = try KudzuJSON.encode(library)
            try data.write(to: url, options: .atomic)
            statusMessage = "Exported \(library.snippets.count) snippet\(library.snippets.count == 1 ? "" : "s") to \(url.lastPathComponent)."
        } catch {
            errorMessage = "Export failed: \(error.localizedDescription)"
        }
    }

    private func exportToEspanso() {
        statusMessage = nil
        errorMessage = nil

        let panel = NSOpenPanel()
        panel.title = "Export to Espanso folder"
        panel.message = "Choose a folder where Espanso YAML match files will be written."
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.canCreateDirectories = true
        panel.allowsMultipleSelection = false

        guard panel.runModal() == .OK, let directoryURL = panel.url else { return }

        let accessed = directoryURL.startAccessingSecurityScopedResource()
        defer {
            if accessed {
                directoryURL.stopAccessingSecurityScopedResource()
            }
        }

        do {
            let library = try model.libraryForTransfer()
            let written = try ImportCoordinator.exportEspanso(library: library, to: directoryURL)
            statusMessage = "Exported \(written.count) Espanso YAML file\(written.count == 1 ? "" : "s") to \(directoryURL.lastPathComponent)."
        } catch {
            errorMessage = "Espanso export failed: \(error.localizedDescription)"
        }
    }
}

// MARK: - About Tab

struct AboutSettingsTab: View {
    private var appVersion: String {
        let shortVersion = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0.1.0"
        let buildNumber = Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "1"
        return "\(shortVersion) (\(buildNumber))"
    }

    @State private var showingLicence = false
    @Environment(\.openURL) private var openURL

    private var fontLicenceURL: URL? {
        let bundle = Bundle.main.url(forResource: "KudzuUI_KudzuUI", withExtension: "bundle")
            .flatMap(Bundle.init(url:))
            ?? Bundle.allBundles.first(where: { $0.bundleURL.lastPathComponent.contains("KudzuUI") })
        return bundle?.url(forResource: "OFL", withExtension: "txt", subdirectory: "Fonts")
            ?? bundle?.url(forResource: "OFL", withExtension: "txt")
    }

    var body: some View {
        SettingsPageLayout(title: "About") {
            VStack(alignment: .leading, spacing: 8) {
                if let url = Bundle.main.url(forResource: "kudzu-app-icon-1024", withExtension: "png"),
                   let image = NSImage(contentsOf: url) {
                    Image(nsImage: image).resizable().frame(width: 64, height: 64)
                }
                Text("Kudzu")
                    .font(WorkspaceTheme.title)
                    .foregroundStyle(WorkspaceTheme.ink)
                Text("A lightweight, native macOS text expander.")
                    .font(WorkspaceTheme.body)
                    .foregroundStyle(WorkspaceTheme.secondary)
                Text("Version \(appVersion) · MIT License")
                    .font(WorkspaceTheme.caption)
                    .foregroundStyle(WorkspaceTheme.secondary)
            }
            WorkspaceSection("Links") {
                WorkspaceSettingRow("Source code", description: "github.com/Stringsaeed/kudzu") {
                    Link("Open", destination: URL(string: "https://github.com/Stringsaeed/kudzu")!)
                        .font(WorkspaceTheme.body)
                }
                WorkspaceSettingRow("Font licence", description: "Geist and Geist Mono · SIL Open Font License") {
                    Button("View…") { showingLicence = true }
                        .controlSize(.small)
                }
            }
        }
        .sheet(isPresented: $showingLicence) {
            VStack(alignment: .leading, spacing: 16) {
                HStack {
                    Text("Font licence").font(WorkspaceTheme.title)
                    Spacer()
                    Button("Done") { showingLicence = false }.keyboardShortcut(.cancelAction)
                }
                ScrollView {
                    Text(fontLicenceURL.flatMap { try? String(contentsOf: $0, encoding: .utf8) }
                         ?? "The bundled Geist OFL notice could not be loaded.")
                        .font(WorkspaceTheme.mono)
                        .textSelection(.enabled)
                }
            }
            .padding(WorkspaceSurface.pagePadding)
            .frame(width: 600, height: 480)
            .background(WorkspaceTheme.paper)
        }
    }
}
