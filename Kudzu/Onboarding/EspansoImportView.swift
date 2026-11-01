import AppKit
import KudzuCore
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
                Label(errorMessage, systemImage: "exclamationmark.octagon.fill")
                    .foregroundStyle(.red)
                    .font(.callout)
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
        GroupBox {
            VStack(alignment: .leading, spacing: 10) {
                Label("Choose Your Espanso Folder", systemImage: "folder.badge.gearshape")
                    .font(.headline)
                Text("Because the Mac App Store version of Kudzu is sandboxed, it needs your permission to read ~/Library/Application Support/espanso or ~/.config/espanso.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)

                HStack {
                    Button("Choose Espanso Folder…") {
                        chooseCustomDirectory()
                    }
                    .buttonStyle(.borderedProminent)
                    Spacer()
                }
            }
            .padding(6)
        }
    }
    #else
    private var directDetectionSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 10) {
                if detectedPreviews.isEmpty {
                    HStack(alignment: .top, spacing: 10) {
                        Image(systemName: "magnifyingglass.circle")
                            .font(.title2)
                            .foregroundStyle(.secondary)
                        VStack(alignment: .leading, spacing: 4) {
                            Text("No Default Espanso Configuration Found")
                                .font(.headline)
                            Text("Checked ~/Library/Application Support/espanso and ~/.config/espanso. You can still choose a custom Espanso match folder below.")
                                .font(.subheadline)
                                .foregroundStyle(.secondary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                } else {
                    Text("Detected Espanso Configurations")
                        .font(.headline)

                    ForEach(detectedPreviews) { preview in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(abbreviatePath(preview.url))
                                    .font(.body.monospaced())
                                Text("\(preview.snippetCount) snippet\(preview.snippetCount == 1 ? "" : "s") in \(preview.groupCount) group\(preview.groupCount == 1 ? "" : "s")")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            if selectedPreview?.url == preview.url {
                                Image(systemName: "checkmark.circle.fill")
                                    .foregroundStyle(.tint)
                            } else {
                                Button("Select") {
                                    selectedPreview = preview
                                    importSummary = nil
                                    errorMessage = nil
                                }
                            }
                        }
                        .padding(.vertical, 4)
                    }
                }

                HStack {
                    Button("Choose Folder…") {
                        chooseCustomDirectory()
                    }
                    if !detectedPreviews.isEmpty {
                        Button("Rescan") {
                            scanDefaultDirectories()
                        }
                    }
                    Spacer()
                }
            }
            .padding(6)
        }
    }
    #endif

    private func previewCard(_ preview: EspansoDirectoryPreview) -> some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Ready to Import")
                            .font(.headline)
                        Text("\(preview.snippetCount) snippet\(preview.snippetCount == 1 ? "" : "s") across \(preview.groupCount) group\(preview.groupCount == 1 ? "" : "s") from \(abbreviatePath(preview.url))")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button("Import \(preview.snippetCount) Snippet\(preview.snippetCount == 1 ? "" : "s") from \(preview.url.lastPathComponent)") {
                        performImport(preview)
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(preview.snippetCount == 0 && preview.result.warnings.isEmpty)
                }

                if !preview.groupedWarnings.isEmpty {
                    WarningGroupsDisclosure(
                        title: "\(preview.result.warnings.count) compatibility warning\(preview.result.warnings.count == 1 ? "" : "s") detected in preview",
                        groups: preview.groupedWarnings
                    )
                }
            }
            .padding(6)
        }
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
        panel.title = "Choose Espanso Configuration or Match Folder"
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
        GroupBox {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(.green)
                        .font(.title3)
                    Text("Import Complete")
                        .font(.headline)
                }

                Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 4) {
                    GridRow {
                        Text("Imported:")
                            .foregroundStyle(.secondary)
                        Text("\(summary.importedSnippets) snippet\(summary.importedSnippets == 1 ? "" : "s"), \(summary.importedGroups) group\(summary.importedGroups == 1 ? "" : "s")")
                            .fontWeight(.medium)
                    }
                    GridRow {
                        Text("Merged duplicates:")
                            .foregroundStyle(.secondary)
                        Text("\(summary.dedupeReport.mergedSnippets) snippet\(summary.dedupeReport.mergedSnippets == 1 ? "" : "s"), \(summary.dedupeReport.mergedGroups) group\(summary.dedupeReport.mergedGroups == 1 ? "" : "s")")
                    }
                    GridRow {
                        Text("Trigger conflicts:")
                            .foregroundStyle(.secondary)
                        if summary.dedupeReport.conflicts.isEmpty {
                            Text("None")
                                .foregroundStyle(.secondary)
                        } else {
                            Text("\(summary.dedupeReport.conflicts.count) (\(summary.dedupeReport.conflicts.map(\.trigger).joined(separator: ", ")))")
                                .foregroundStyle(.orange)
                        }
                    }
                    GridRow {
                        Text("Safety backup:")
                            .foregroundStyle(.secondary)
                        Text(summary.backupURL.lastPathComponent)
                            .font(.caption.monospaced())
                            .foregroundStyle(.secondary)
                    }
                }
                .font(.subheadline)

                if !summary.groupedWarnings.isEmpty {
                    WarningGroupsDisclosure(
                        title: "\(summary.warnings.count) warning\(summary.warnings.count == 1 ? "" : "s") (\(summary.groupedWarnings.count) type\(summary.groupedWarnings.count == 1 ? "" : "s"))",
                        groups: summary.groupedWarnings
                    )
                }
            }
            .padding(6)
        }
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
                                    .font(.caption.monospaced().bold())
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(.quaternary, in: Capsule())
                                Text(group.message)
                                    .font(.caption)
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
                                    .font(.caption2.monospaced())
                                    .foregroundStyle(.secondary)
                                    .padding(.leading, 32)
                            }
                        }
                    }
                }
                .padding(.top, 6)
            }
            .frame(maxHeight: 140)
        } label: {
            Label(title, systemImage: "exclamationmark.triangle.fill")
                .font(.subheadline)
                .foregroundStyle(.orange)
        }
    }
}
