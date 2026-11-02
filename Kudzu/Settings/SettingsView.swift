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
        }.background(KzColor.surface)
    }

    private var pageContent: some View {
        VStack(alignment: .leading, spacing: KzSpace.s6) {
            if !inModal || description != nil {
                VStack(alignment: .leading, spacing: KzSpace.s1) {
                    if !inModal {
                        Text(title).kzTextStyle(.titleLg).foregroundStyle(KzColor.ink)
                            .accessibilityAddTraits(.isHeader)
                    }
                    if let description {
                        Text(description).kzTextStyle(.body).foregroundStyle(KzColor.inkMuted)
                    }
                }
            }
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, KzSpace.s6)
        .padding(.top, inModal ? KzSpace.s4 : KzSpace.s8)
        .padding(.bottom, KzSpace.s6)
    }
}

struct SettingsMessage: View {
    var text: String
    var isError = false
    var body: some View {
        HStack(alignment: .top, spacing: KzSpace.s2) {
            KzIcon(isError ? .error : .checkCircle, size: 14)
            Text(text).kzTextStyle(.callout).textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
        }
        .foregroundStyle(isError ? KzColor.danger : KzColor.inkMuted)
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
            KzSettingsGroup("Startup") {
                KzSettingsRow("Launch Kudzu at login") {
                    KzToggle("Launch Kudzu at login", isOn: Binding(
                        get: { launchAtLogin }, set: { setLaunchAtLogin($0) }
                    ))
                }
                if launchAtLoginRequiresApproval {
                    SettingsMessage(text: "Requires approval in System Settings → General → Login Items.")
                        .padding(KzSpace.s4)
                }
                if let launchAtLoginError {
                    SettingsMessage(text: launchAtLoginError, isError: true).padding(KzSpace.s4)
                }
            }
            KzSettingsGroup("Snippets & stats") {
                KzSettingsRow("Typing speed", description: "Used to estimate time saved") {
                    KzStepper("Typing speed", value: Binding(
                        get: { typingSpeedWPM }, set: { updateTypingSpeed($0) }
                    ), step: 5, min: 10, max: 240, unit: "wpm")
                }
                KzSettingsRow("Default trigger prefix", description: "Added to new triggers you type") {
                    KzTextInput("Prefix", text: Binding(
                        get: { defaultTriggerPrefix }, set: { updateDefaultTriggerPrefix($0) }
                    ), mono: true).frame(width: 64).accessibilityLabel("Default trigger prefix")
                }
                KzSettingsRow("Search palette shortcut") { PaletteShortcutSetting() }
            }
            KzSettingsGroup("Onboarding") {
                KzSettingsRow("Setup assistant") {
                    KzButton("Show welcome again", size: .sm) {
                        openWindow(id: SceneID.onboarding)
                        NSApplication.shared.activate()
                    }
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
            KzSettingsGroup("iCloud") {
                KzSettingsRow(description: phase.statusText, label: {
                    Text("iCloud sync").kzTextStyle(.bodyStrong)
                }, control: {
                    KzToggle("iCloud sync", isOn: Binding(
                        get: {
                            model.syncMonitor.snapshot.blocked == nil
                                && model.syncMonitor.snapshot.preferenceEnabled
                        },
                        set: { model.setCloudSyncEnabled($0) }
                    ))
                    .disabled(model.syncMonitor.snapshot.blocked != nil)
                })
                if case .upToDate(let date?) = phase {
                    KzSettingsRow("Last synced") {
                        Text(date.formatted(date: .abbreviated, time: .shortened))
                            .kzTextStyle(.monoCaption).foregroundStyle(KzColor.inkMuted)
                    }
                }
                if case .needsRestart = phase {
                    KzSettingsRow("Apply the change", description: "The library that's open keeps its current sync mode.") {
                        KzButton("Quit and reopen", size: .sm) { NSApp.terminate(nil) }
                    }
                }
            }
            KzSettingsGroup("This Mac") {
                KzSettingsRow("Device ID", description: "Usage is counted separately on each Mac, then added together.") {
                    Text(deviceID.isEmpty ? "—" : deviceID)
                        .kzTextStyle(.monoCaption).foregroundStyle(KzColor.inkMuted)
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
            KzSettingsGroup("Snapshots", description: "Keeps the last 14 snapshots.", action: {
                KzButton("Back up now", variant: .primary, size: .sm, icon: .history) { createManualBackup() }
            }) {
                if rows.isEmpty {
                    KzSettingsRow("No backups yet", description: "Choose Back up now to save your first snapshot.") { EmptyView() }
                } else {
                    if snapshotRendering {
                        VStack(spacing: 0) { backupRows }.padding(KzSpace.s1)
                    } else {
                        List(selection: $selectedBackupURL) { backupRows }
                            .kzSelectionList()
                            .frame(height: min(360, CGFloat(rows.count) * 64 + 8))
                    }
                }
            }
            HStack(spacing: KzSpace.s2) {
                KzButton("Restore…", size: .sm) { backupPendingRestore = selectedRow }
                    .disabled(selectedRow == nil)
                KzButton("Reveal in Finder", variant: .ghost, size: .sm) { revealInFinder() }
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
            KzListRow(
                row.info.createdAt.formatted(date: .abbreviated, time: .standard),
                meta: row.info.filename,
                aside: row.snippetCount.map { "\($0) snippet\($0 == 1 ? "" : "s")" },
                isSelected: selectedBackupURL == row.info.url
            ) {
                KzBadge(reasonLabel(row.info.reason), tone: row.info.reason == .preImport ? .warning : .neutral)
            }
            .kzListRow().tag(row.info.url)
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
            let library = try model.store.library()
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
            KzSettingsGroup("Import", description: "A backup is taken before every import.") {
                KzSettingsRow("Espanso YAML", description: "match/*.yml files") {
                    KzButton("Import…", size: .sm) { showingEspansoSheet = true }
                }
                KzSettingsRow("Kudzu JSON", description: "A library exported from Kudzu") {
                    KzButton("Import…", size: .sm) { importKudzuLibrary() }
                }
            }
            KzSettingsGroup("Export") {
                KzSettingsRow("Kudzu JSON", description: "Lossless: rules, stats and groups included") {
                    KzButton("Export…", size: .sm) { exportKudzuLibrary() }
                }
                KzSettingsRow("Espanso YAML", description: "Triggers and content only") {
                    KzButton("Export…", size: .sm) { exportToEspanso() }
                }
            }
            if let summary = lastImportSummary {
                KzSettingsGroup("Latest import result") { ImportSummaryCard(summary: summary).padding(KzSpace.s4) }
            }
            if let statusMessage { SettingsMessage(text: statusMessage) }
            if let errorMessage { SettingsMessage(text: errorMessage, isError: true) }
        }
        .sheet(isPresented: $showingEspansoSheet) {
            VStack(alignment: .leading, spacing: 16) {
                HStack {
                    Text("Import from Espanso")
                        .kzTextStyle(.title)
                    Spacer()
                    KzButton("Done", size: .sm) {
                        showingEspansoSheet = false
                    }
                    .keyboardShortcut(.cancelAction)
                }
                Divider()
                ScrollView {
                    EspansoImportView()
                        .environment(model)
                }
            }
            .padding(20)
            .frame(width: 540, height: 380)
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
            let library = try model.store.library()
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
            let library = try model.store.library()
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
            VStack(alignment: .leading, spacing: KzSpace.s2) {
                if let url = Bundle.main.url(forResource: "kudzu-app-icon-1024", withExtension: "png"),
                   let image = NSImage(contentsOf: url) {
                    Image(nsImage: image).resizable().frame(width: 80, height: 80)
                }
                Text("Kudzu").kzTextStyle(.titleLg).foregroundStyle(KzColor.ink)
                Text("A lightweight, native macOS text expander.").kzTextStyle(.body).foregroundStyle(KzColor.inkMuted)
                Text("Version \(appVersion) · MIT License").kzTextStyle(.monoCaption).foregroundStyle(KzColor.inkMuted)
            }
            KzSettingsGroup {
                KzSettingsRow("Source code", description: "github.com/Stringsaeed/kudzu") {
                    KzButton("Open", variant: .ghost, size: .sm, iconEnd: .external) {
                        openURL(URL(string: "https://github.com/Stringsaeed/kudzu")!)
                    }
                }
                KzSettingsRow("Font licence", description: "Geist and Geist Mono · SIL Open Font License") {
                    KzButton("View…", size: .sm) { showingLicence = true }
                }
            }
        }
        .sheet(isPresented: $showingLicence) {
            VStack(alignment: .leading, spacing: KzSpace.s4) {
                HStack {
                    Text("Font licence").kzTextStyle(.title)
                    Spacer()
                    KzButton("Done", size: .sm) { showingLicence = false }.keyboardShortcut(.cancelAction)
                }
                ScrollView {
                    Text(fontLicenceURL.flatMap { try? String(contentsOf: $0, encoding: .utf8) }
                         ?? "The bundled Geist OFL notice could not be loaded.")
                        .kzTextStyle(.monoCaption).textSelection(.enabled)
                }
            }.padding(KzSpace.s6).frame(width: 600, height: 480).background(KzColor.surface)
        }
    }
}
