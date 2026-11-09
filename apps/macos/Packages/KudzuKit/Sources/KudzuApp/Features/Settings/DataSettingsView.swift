import KudzuCore
import KudzuDesign
import SwiftUI

/// Backups & data (`settings.tsx` `Data`). Import and export go through `DataActions` and the model.
struct DataSettingsView: View {
    @Environment(AppModel.self) private var model
    @State private var backups: [BackupInfo] = []
    @State private var espanso: [EspansoDetection] = []

    var body: some View {
        VStack(alignment: .leading, spacing: 32) {
            SettingsCardGroup(
                "Import",
                description: "Bring snippets from Raycast, TextExpander, Espanso or another Kudzu library. A backup is saved first."
            ) {
                ForEach(espanso, id: \.path) { detected in
                    SettingsRow(
                        "Espanso found on this Mac",
                        description: DataCopy.espansoFound(detected)
                    ) {
                        Button {
                            Task { await importEspanso(detected) }
                        } label: {
                            ButtonLabel("Import", icon: Icon.download)
                        }
                        .buttonStyle(.kz(.default, size: .sm))
                        .accessibilityLabel("Import Espanso snippets from \(detected.path)")
                    }
                    Hairline()
                }

                SettingsRow("Raycast", description: DataActions.raycastHint) {
                    Button("Choose file…") { Task { await DataActions.runImport(.raycast, model: model) } }
                        .buttonStyle(.kz(.secondary, size: .sm))
                        .accessibilityLabel("Choose file… for Raycast")
                }

                Hairline()
                SettingsRow("TextExpander", description: DataActions.textExpanderHint) {
                    Button("Choose file…") { Task { await DataActions.runImport(.textexpander, model: model) } }
                        .buttonStyle(.kz(.secondary, size: .sm))
                        .accessibilityLabel("Choose file… for TextExpander")
                }

                Hairline()
                SettingsRow("Kudzu or Espanso", description: "A Kudzu .json library, or Espanso .yml files and folders.") {
                    HStack(spacing: 8) {
                        Button("Choose file…") { Task { await DataActions.runImport(.file, model: model) } }
                            .buttonStyle(.kz(.secondary, size: .sm))
                            .accessibilityLabel("Choose file… for Kudzu or Espanso")
                        Button("Espanso folder…") { Task { await DataActions.runImport(.espansoFolder, model: model) } }
                            .buttonStyle(.kz(.ghost, size: .sm))
                    }
                }
            }
            .stagger(0)

            SettingsCardGroup("Export", description: "Your snippets are always yours to take elsewhere.") {
                SettingsRow("Kudzu library", description: "Everything, lossless — snippets, groups, rules and usage.") {
                    Button {
                        Task { await DataActions.runExport(.json, model: model) }
                    } label: {
                        ButtonLabel("Export JSON", icon: Icon.upload)
                    }
                    .buttonStyle(.kz(.secondary, size: .sm))
                }
                Hairline()
                SettingsRow("Espanso", description: "Best-effort YAML so you can switch back any time.") {
                    Button {
                        Task { await DataActions.runExport(.espanso, model: model) }
                    } label: {
                        ButtonLabel("Export YAML", icon: Icon.upload)
                    }
                    .buttonStyle(.kz(.secondary, size: .sm))
                }
            }
            .stagger(1)

            SettingsCardGroup("Backups", description: "Kudzu keeps a snapshot every day and before every import — the last 14.") {

                if backups.isEmpty {
                    Text("No backups yet.")
                        .font(.system(size: 12))
                        .mutedForeground()
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.vertical, 16)
                    Hairline()
                }
                ForEach(backups.prefix(14), id: \.path) { backup in
                    HStack(spacing: 12) {
                        Icon.history.icon(16).foregroundStyle(.kz.faint)
                        VStack(alignment: .leading, spacing: 0) {
                            Text(DataCopy.backupDate(backup.createdAt))
                                .font(.system(size: 13))
                                .foregroundStyle(.kz.foreground)
                            Text(DataCopy.backupDetail(backup))
                                .font(.system(size: 11))
                                .foregroundStyle(.kz.faint)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        Button {
                            Task { await restore(backup) }
                        } label: {
                            ButtonLabel("Restore", icon: Icon.rotateCcw)
                        }
                        .buttonStyle(.kz(.ghost, size: .sm))
                    }
                    .padding(.vertical, 10)
                    Hairline()
                }

                HStack(spacing: 8) {
                    Button("Back up now") {
                        Task {
                            if let updated = await model.createManualBackup() {
                                backups = updated
                            }
                        }
                    }
                    .buttonStyle(.kz(.secondary, size: .sm))
                    Button {
                        Task { await model.revealDataDirectory() }
                    } label: {
                        ButtonLabel("Show data folder", icon: Icon.folderOpen)
                    }
                    .buttonStyle(.kz(.ghost, size: .sm))
                }
                .padding(.vertical, 12)
            }
            .stagger(2)
        }
        .task { await reload() }
    }

    /// Restores at once, like the web: the backend saves the current library as a "Before restore"
    /// backup first, so the restore can itself be undone from this list, and the toast says so.
    private func restore(_ backup: BackupInfo) async {
        if let updated = await model.restoreFromBackup(backup, dateLabel: DataCopy.backupDate(backup.createdAt)) {
            backups = updated
        }
    }

    private func importEspanso(_ detected: EspansoDetection) async {
        do {
            let summary = try await model.importDetectedEspanso(path: detected.path)
            toast.success("Imported \(summary.importedSnippets) snippets")
            await reload()
        } catch {
            toast.error("Import failed", description: AppModel.errorMessage(error))
        }
    }

    private func reload() async {
        let loaded = await model.loadDataSources()
        backups = loaded.backups
        espanso = loaded.espanso
    }
}
