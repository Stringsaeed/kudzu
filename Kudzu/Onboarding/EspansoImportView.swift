import AppKit
import KudzuCore
import KudzuUI
import SwiftUI

/// Reusable view that previews and imports an Espanso configuration directory into `KudzuStore`.
///
/// Used in Onboarding Step 2 and in Settings → Import & Export.
struct EspansoImportView: View {
    @Environment(AppModel.self) private var model

    @State private var detectedPreviews: [EspansoDirectoryPreview] = []
    @State private var selectedPreview: EspansoDirectoryPreview?
    @State private var importSummary: ImportSummary?
    @State private var errorMessage: String?
    @State private var hasScanned = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            #if APPSTORE
            appStorePickerSection
            #else
            directDetectionSection
            #endif

            if let preview = selectedPreview, importSummary == nil {
                previewCard(preview)
            }

            if let summary = importSummary {
                ImportSummaryCard(summary: summary)
            }

            if let errorMessage {
                SettingsMessage(text: errorMessage, isError: true)
            }
        }
        .onAppear {
            guard !hasScanned else { return }
            hasScanned = true
            #if !APPSTORE
            scanDefaultDirectories()
            #endif
        }
    }

    // MARK: - Sections

    #if APPSTORE
    private var appStorePickerSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Choose your Espanso folder")
                .font(WorkspaceTheme.label)
            Text("Because the Mac App Store version of Kudzu is sandboxed, it needs your permission to read ~/Library/Application Support/espanso or ~/.config/espanso.")
                .font(WorkspaceTheme.body)
                .foregroundStyle(WorkspaceTheme.secondary)
                .fixedSize(horizontal: false, vertical: true)

            Button("Choose Espanso folder…") { chooseCustomDirectory() }
                .controlSize(.small)
        }
    }
    #else
    private var directDetectionSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            if detectedPreviews.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("No default Espanso configuration found")
                        .font(WorkspaceTheme.label)
                    Text("Checked ~/Library/Application Support/espanso and ~/.config/espanso. You can still choose a custom Espanso match folder below.")
                        .font(WorkspaceTheme.body)
                        .foregroundStyle(WorkspaceTheme.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            } else {
                Text("Detected Espanso configurations")
                    .font(WorkspaceTheme.label)

                ForEach(detectedPreviews) { preview in
                    HStack(alignment: .center) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(abbreviatePath(preview.url))
                                .font(WorkspaceTheme.mono)
                            Text("\(preview.snippetCount) snippet\(preview.snippetCount == 1 ? "" : "s") in \(preview.groupCount) group\(preview.groupCount == 1 ? "" : "s")")
                                .font(WorkspaceTheme.caption)
                                .foregroundStyle(WorkspaceTheme.secondary)
                        }
                        Spacer()
                        if selectedPreview?.url == preview.url {
                            Text("Selected")
                                .font(WorkspaceTheme.caption)
                                .foregroundStyle(WorkspaceTheme.accent)
                        } else {
                            Button("Select") {
                                selectedPreview = preview
                                importSummary = nil
                                errorMessage = nil
                            }
                            .controlSize(.small)
                        }
                    }
                    .padding(.vertical, 6)
                    .overlay(alignment: .bottom) { WorkspaceHairline() }
                }
            }

            HStack {
                Button("Choose folder…") { chooseCustomDirectory() }
                    .controlSize(.small)
                if !detectedPreviews.isEmpty {
                    Button("Rescan") { scanDefaultDirectories() }
                        .controlSize(.small)
                        .buttonStyle(.borderless)
                }
                Spacer()
            }
        }
    }
    #endif

    private func previewCard(_ preview: EspansoDirectoryPreview) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Ready to import")
                        .font(WorkspaceTheme.label)
                    Text("\(preview.snippetCount) snippet\(preview.snippetCount == 1 ? "" : "s") across \(preview.groupCount) group\(preview.groupCount == 1 ? "" : "s") from \(abbreviatePath(preview.url))")
                        .font(WorkspaceTheme.body)
                        .foregroundStyle(WorkspaceTheme.secondary)
                }
                Spacer(minLength: 12)
                Button("Import \(preview.snippetCount) snippet\(preview.snippetCount == 1 ? "" : "s")") {
                    performImport(preview)
                }
                .controlSize(.small)
                .disabled(preview.snippetCount == 0 && preview.result.warnings.isEmpty)
            }

            if !preview.groupedWarnings.isEmpty {
                WarningGroupsDisclosure(
                    title: "\(preview.result.warnings.count) compatibility warning\(preview.result.warnings.count == 1 ? "" : "s") detected in preview",
                    groups: preview.groupedWarnings
                )
            }
        }
        .padding(.top, 8)
    }

    // MARK: - Actions

    private func scanDefaultDirectories() {
        errorMessage = nil
        let found = ImportCoordinator.detectEspansoDirectories()
        detectedPreviews = found
        if selectedPreview == nil {
            selectedPreview = found.first(where: { $0.snippetCount > 0 }) ?? found.first
        }
    }

    private func chooseCustomDirectory() {
        errorMessage = nil
        let panel = NSOpenPanel()
        panel.title = "Choose Espanso configuration or match folder"
        panel.message = "Select your espanso directory (e.g. ~/Library/Application Support/espanso)."
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.showsHiddenFiles = true

        if let firstCandidate = EspansoImporter.defaultConfigDirectories().first {
            let parent = firstCandidate.deletingLastPathComponent()
            if FileManager.default.fileExists(atPath: parent.path) {
                panel.directoryURL = parent
            }
        }

        guard panel.runModal() == .OK, let url = panel.url else { return }

        let accessed = url.startAccessingSecurityScopedResource()
        defer {
            if accessed {
                url.stopAccessingSecurityScopedResource()
            }
        }

        do {
            let preview = try ImportCoordinator.previewEspansoDirectory(url)
            selectedPreview = preview
            importSummary = nil
        } catch {
            errorMessage = "Couldn't read Espanso directory: \(error.localizedDescription)"
        }
    }

    private func performImport(_ preview: EspansoDirectoryPreview) {
        errorMessage = nil
        let accessed = preview.url.startAccessingSecurityScopedResource()
        defer {
            if accessed {
                preview.url.stopAccessingSecurityScopedResource()
            }
        }

        do {
            _ = try model.libraryForTransfer()
            importSummary = try ImportCoordinator.importEspanso(
                result: preview.result,
                into: model.store,
                backupStore: model.backupStore
            )
        } catch {
            errorMessage = "Import failed: \(error.localizedDescription)"
        }
    }

    private func abbreviatePath(_ url: URL) -> String {
        (url.path as NSString).abbreviatingWithTildeInPath
    }
}

/// Displays the results of an import: imported count, merged duplicates, conflicts, and expandable grouped warnings.
struct ImportSummaryCard: View {
    let summary: ImportSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Import complete")
                .font(WorkspaceTheme.label)

            Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 4) {
                GridRow {
                    Text("Imported:")
                        .foregroundStyle(WorkspaceTheme.secondary)
                    Text("\(summary.importedSnippets) snippet\(summary.importedSnippets == 1 ? "" : "s"), \(summary.importedGroups) group\(summary.importedGroups == 1 ? "" : "s")")
                        .font(WorkspaceTheme.body.weight(.medium))
                }
                GridRow {
                    Text("Merged duplicates:")
                        .foregroundStyle(WorkspaceTheme.secondary)
                    Text("\(summary.dedupeReport.mergedSnippets) snippet\(summary.dedupeReport.mergedSnippets == 1 ? "" : "s"), \(summary.dedupeReport.mergedGroups) group\(summary.dedupeReport.mergedGroups == 1 ? "" : "s")")
                        .font(WorkspaceTheme.body)
                }
                GridRow {
                    Text("Trigger conflicts:")
                        .foregroundStyle(WorkspaceTheme.secondary)
                    if summary.dedupeReport.conflicts.isEmpty {
                        Text("None")
                            .foregroundStyle(WorkspaceTheme.secondary)
                    } else {
                        Text("\(summary.dedupeReport.conflicts.count) (\(summary.dedupeReport.conflicts.map(\.trigger).joined(separator: ", ")))")
                            .foregroundStyle(KzColor.warning)
                            .font(WorkspaceTheme.body)
                    }
                }
                GridRow {
                    Text("Safety backup:")
                        .foregroundStyle(WorkspaceTheme.secondary)
                    Text(summary.backupURL.lastPathComponent)
                        .font(WorkspaceTheme.mono)
                        .foregroundStyle(WorkspaceTheme.secondary)
                }
            }

            if !summary.groupedWarnings.isEmpty {
                WarningGroupsDisclosure(
                    title: "\(summary.warnings.count) warning\(summary.warnings.count == 1 ? "" : "s") (\(summary.groupedWarnings.count) type\(summary.groupedWarnings.count == 1 ? "" : "s"))",
                    groups: summary.groupedWarnings
                )
            }
        }
        .padding(.top, 8)
    }
}

/// Expandable list of warnings grouped by message.
struct WarningGroupsDisclosure: View {
    let title: String
    let groups: [WarningGroup]
    @State private var isExpanded = false

    var body: some View {
        DisclosureGroup(isExpanded: $isExpanded) {
            ScrollView {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(groups) { group in
                        VStack(alignment: .leading, spacing: 2) {
                            HStack(alignment: .firstTextBaseline) {
                                Text("×\(group.count)")
                                    .font(WorkspaceTheme.mono)
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(WorkspaceTheme.line.opacity(0.4), in: RoundedRectangle(cornerRadius: 4))
                                Text(group.message)
                                    .font(WorkspaceTheme.caption)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                            let locations = group.warnings.prefix(5).map { warning in
                                var parts = [warning.file]
                                if let line = warning.line { parts.append("L\(line)") }
                                if let trigger = warning.trigger { parts.append("(\(trigger))") }
                                return parts.joined(separator: ":")
                            }.joined(separator: ", ")
                            if !locations.isEmpty {
                                Text(locations + (group.warnings.count > 5 ? ", …" : ""))
                                    .font(WorkspaceTheme.mono)
                                    .foregroundStyle(WorkspaceTheme.secondary)
                                    .padding(.leading, 32)
                            }
                        }
                    }
                }
                .padding(.top, 6)
            }
            .frame(maxHeight: 140)
        } label: {
            Text(title)
                .font(WorkspaceTheme.body)
                .foregroundStyle(KzColor.warning)
        }
    }
}
