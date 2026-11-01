import AppKit
import KudzuCore
import ServiceManagement
import SwiftUI
import UniformTypeIdentifiers

/// The Settings window (⌘,).
struct SettingsView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        TabView {
            GeneralSettingsTab()
                .tabItem {
                    Label("General", systemImage: "gearshape")
                }

            SyncSettingsTab()
                .tabItem {
                    Label("Sync", systemImage: "icloud")
                }

            BackupsSettingsTab()
                .tabItem {
                    Label("Backups", systemImage: "clock.arrow.circlepath")
                }

            ImportExportSettingsTab()
                .tabItem {
                    Label("Import & Export", systemImage: "arrow.up.arrow.down.square")
                }

            AboutSettingsTab()
                .tabItem {
                    Label("About", systemImage: "info.circle")
                }
        }
        .frame(width: 600, height: 440)
        .onAppear {
            NSApplication.shared.activate()
        }
    }
}

// MARK: - General Tab

private struct GeneralSettingsTab: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openWindow) private var openWindow

    @State private var launchAtLogin = false
    @State private var launchAtLoginRequiresApproval = false
    @State private var launchAtLoginError: String?
    @State private var typingSpeedWPM = 40
    @State private var defaultTriggerPrefix = ";"
    @State private var settingsError: String?

    var body: some View {
        Form {
            Section("Startup") {
                Toggle("Launch Kudzu at login", isOn: Binding(
                    get: { launchAtLogin },
                    set: { setLaunchAtLogin($0) }
                ))

                if launchAtLoginRequiresApproval {
                    Text("Requires approval in System Settings → General → Login Items.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                if let launchAtLoginError {
                    Text(launchAtLoginError)
                        .font(.caption)
                        .foregroundStyle(.red)
                }
            }

            Section("Snippets & Stats") {
                Stepper(
                    value: Binding(
                        get: { typingSpeedWPM },
                        set: { updateTypingSpeed($0) }
                    ),
                    in: 10...240,
                    step: 5
                ) {
                    LabeledContent("Typing speed") {
                        Text("\(typingSpeedWPM) WPM")
                            .monospacedDigit()
                    }
                }

                LabeledContent("Default trigger prefix") {
                    TextField("Prefix", text: Binding(
                        get: { defaultTriggerPrefix },
                        set: { updateDefaultTriggerPrefix($0) }
                    ))
                    .frame(width: 80)
                    .textFieldStyle(.roundedBorder)
                }

                LabeledContent("Search palette shortcut") { PaletteShortcutSetting() }
            }

            Section("Onboarding") {
                LabeledContent("Setup assistant") {
                    Button("Show Welcome Again…") {
                        openWindow(id: SceneID.onboarding)
                        NSApplication.shared.activate()
                    }
                }
            }

            if let settingsError {
                Section {
                    Text(settingsError)
                        .font(.caption)
                        .foregroundStyle(.red)
                }
            }
        }
        .formStyle(.grouped)
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

private struct SyncSettingsTab: View {
    @Environment(AppModel.self) private var model
    @State private var deviceID = ""

    var body: some View {
        Form {
            Section("CloudKit Sync") {
                Toggle(isOn: .constant(false)) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("iCloud Sync")
                        Text("Coming soon — automatic CloudKit sync across your Macs lands in Milestone 4.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                .disabled(true)
            }

            Section("Device") {
                LabeledContent("Device ID") {
                    Text(deviceID.isEmpty ? "—" : deviceID)
                        .font(.caption.monospaced())
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)
                }
            }
        }
        .formStyle(.grouped)
        .onAppear {
            if let local = try? model.store.localSettings() {
                deviceID = local.deviceID
            }
        }
    }
}

// MARK: - Backups Tab

private struct BackupsSettingsTab: View {
    struct BackupRow: Identifiable, Hashable {
        var info: BackupInfo
        var snippetCount: Int?
        var id: URL { info.url }
    }

    @Environment(AppModel.self) private var model

    @State private var rows: [BackupRow] = []
    @State private var selectedBackupURL: URL?
    @State private var backupPendingRestore: BackupRow?
    @State private var statusMessage: String?
    @State private var errorMessage: String?

    private var selectedRow: BackupRow? {
        rows.first { $0.info.url == selectedBackupURL }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Local Library Backups")
                        .font(.headline)
                    Text("Automatic backups run daily, keeping the last 14 snapshots (plus a snapshot before every import).")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button("Back Up Now") {
                    createManualBackup()
                }
                .buttonStyle(.borderedProminent)
            }

            List(selection: $selectedBackupURL) {
                if rows.isEmpty {
                    ContentUnavailableView(
                        "No Backups Yet",
                        systemImage: "clock.arrow.circlepath",
                        description: Text("Click “Back Up Now” to create a snapshot immediately.")
                    )
                } else {
                    ForEach(rows) { row in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(row.info.createdAt.formatted(date: .abbreviated, time: .standard))
                                    .font(.body.weight(.medium))
                                Text(row.info.filename)
                                    .font(.caption2.monospaced())
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            if let count = row.snippetCount {
                                Text("\(count) snippet\(count == 1 ? "" : "s")")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            Text(reasonLabel(row.info.reason))
                                .font(.caption.weight(.medium))
                                .padding(.horizontal, 8)
                                .padding(.vertical, 2)
                                .background(.quaternary, in: Capsule())
                        }
                        .tag(row.info.url)
                    }
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: 6))

            HStack {
                Button("Restore…") {
                    backupPendingRestore = selectedRow
                }
                .disabled(selectedRow == nil)

                Button("Reveal in Finder") {
                    revealInFinder()
                }

                Spacer()

                if let statusMessage {
                    Text(statusMessage)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                if let errorMessage {
                    Text(errorMessage)
                        .font(.caption)
                        .foregroundStyle(.red)
                }
            }
        }
        .padding(20)
        .onAppear {
            reloadBackups()
        }
        .alert(
            "Restore Backup?",
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

    private func reasonLabel(_ reason: BackupReason) -> String {
        switch reason {
        case .daily: "Daily"
        case .preImport: "Pre-Import"
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

private struct ImportExportSettingsTab: View {
    @Environment(AppModel.self) private var model

    @State private var showingEspansoSheet = false
    @State private var lastImportSummary: ImportSummary?
    @State private var statusMessage: String?
    @State private var errorMessage: String?

    var body: some View {
        Form {
            Section("Import") {
                LabeledContent("Espanso YAML") {
                    Button("Import from Espanso…") {
                        showingEspansoSheet = true
                    }
                }

                LabeledContent("Kudzu JSON") {
                    Button("Import Kudzu Library…") {
                        importKudzuLibrary()
                    }
                }
            }

            Section("Export") {
                LabeledContent("Kudzu JSON (lossless)") {
                    Button("Export Kudzu Library…") {
                        exportKudzuLibrary()
                    }
                }

                LabeledContent("Espanso YAML") {
                    Button("Export to Espanso…") {
                        exportToEspanso()
                    }
                }
            }

            if let summary = lastImportSummary {
                Section("Latest Import Result") {
                    ImportSummaryCard(summary: summary)
                }
            }

            if let statusMessage {
                Section {
                    Label(statusMessage, systemImage: "checkmark.circle.fill")
                        .foregroundStyle(.green)
                        .font(.callout)
                }
            }

            if let errorMessage {
                Section {
                    Label(errorMessage, systemImage: "exclamationmark.octagon.fill")
                        .foregroundStyle(.red)
                        .font(.callout)
                }
            }
        }
        .formStyle(.grouped)
        .sheet(isPresented: $showingEspansoSheet) {
            VStack(alignment: .leading, spacing: 16) {
                HStack {
                    Text("Import from Espanso")
                        .font(.title3.bold())
                    Spacer()
                    Button("Done") {
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
        panel.title = "Import Kudzu Library"
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
        panel.title = "Export Kudzu Library"
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
        panel.title = "Export to Espanso Folder"
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

private struct AboutSettingsTab: View {
    private var appVersion: String {
        let shortVersion = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0.1.0"
        let buildNumber = Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "1"
        return "\(shortVersion) (\(buildNumber))"
    }

    var body: some View {
        VStack(spacing: 16) {
            Spacer()

            Image(systemName: "text.badge.plus")
                .font(.system(size: 52, weight: .semibold))
                .foregroundStyle(.tint)

            VStack(spacing: 4) {
                Text("Kudzu")
                    .font(.title.bold())
                Text("Version \(appVersion)")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            Text("A lightweight, native macOS text expander.")
                .font(.body)
                .foregroundStyle(.secondary)

            VStack(spacing: 6) {
                Text("Released under the MIT License.")
                    .font(.callout)
                    .foregroundStyle(.secondary)

                if let repoURL = URL(string: "https://github.com/Stringsaeed/kudzu") {
                    Link("github.com/Stringsaeed/kudzu", destination: repoURL)
                        .font(.callout.weight(.medium))
                }
            }

            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(24)
    }
}
