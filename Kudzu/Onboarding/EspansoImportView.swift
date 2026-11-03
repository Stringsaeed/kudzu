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
                    .foregroundStyle(KzColor.danger)
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
        KzSettingsGroup {
            VStack(alignment: .leading, spacing: 10) {
                Text("Choose your Espanso folder")
                    .kzTextStyle(.headline)
                Text("Because the Mac App Store version of Kudzu is sandboxed, it needs your permission to read ~/Library/Application Support/espanso or ~/.config/espanso.")
                    .kzTextStyle(.body)
                    .foregroundStyle(KzColor.inkMuted)
                    .fixedSize(horizontal: false, vertical: true)

                HStack {
                    KzButton("Choose Espanso folder…") {
                        chooseCustomDirectory()
                    }

                    Spacer()
                }
            }
            .padding(6)
        }
    }
    #else
    private var directDetectionSection: some View {
        KzSettingsGroup {
            VStack(alignment: .leading, spacing: 10) {
                if detectedPreviews.isEmpty {
                    HStack(alignment: .top, spacing: 10) {
                        KzIcon(.search)
                            .kzTextStyle(.title)
                            .foregroundStyle(KzColor.inkMuted)
                        VStack(alignment: .leading, spacing: 4) {
                            Text("No default Espanso configuration found")
                                .kzTextStyle(.headline)
                            Text("Checked ~/Library/Application Support/espanso and ~/.config/espanso. You can still choose a custom Espanso match folder below.")
                                .kzTextStyle(.body)
                                .foregroundStyle(KzColor.inkMuted)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                } else {
                    Text("Detected Espanso configurations")
                        .kzTextStyle(.headline)

                    ForEach(detectedPreviews) { preview in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(abbreviatePath(preview.url))
                                    .kzTextStyle(.monoCaption)
                                Text("\(preview.snippetCount) snippet\(preview.snippetCount == 1 ? "" : "s") in \(preview.groupCount) group\(preview.groupCount == 1 ? "" : "s")")
                                    .kzTextStyle(.caption)
                                    .foregroundStyle(KzColor.inkMuted)
                            }
                            Spacer()
                            if selectedPreview?.url == preview.url {
                                KzIcon(.checkCircle)
                                    .foregroundStyle(KzColor.vineInk)
                            } else {
                                KzButton("Select") {
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
                    KzButton("Choose folder…") {
                        chooseCustomDirectory()
                    }
                    if !detectedPreviews.isEmpty {
                        KzButton("Rescan") {
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
        KzSettingsGroup {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Ready to import")
                            .kzTextStyle(.headline)
                        Text("\(preview.snippetCount) snippet\(preview.snippetCount == 1 ? "" : "s") across \(preview.groupCount) group\(preview.groupCount == 1 ? "" : "s") from \(abbreviatePath(preview.url))")
                            .kzTextStyle(.body)
                            .foregroundStyle(KzColor.inkMuted)
                    }
                    Spacer()
                    KzButton("Import \(preview.snippetCount) snippet\(preview.snippetCount == 1 ? "" : "s") from \(preview.url.lastPathComponent)") {
                        performImport(preview)
                    }

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
            _ = try model.library.libraryForTransfer()
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
        KzSettingsGroup {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    KzIcon(.checkCircle)
                        .foregroundStyle(KzColor.inkMuted)
                        .kzTextStyle(.title)
                    Text("Import complete")
                        .kzTextStyle(.headline)
                }

                Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 4) {
                    GridRow {
                        Text("Imported:")
                            .foregroundStyle(KzColor.inkMuted)
                        Text("\(summary.importedSnippets) snippet\(summary.importedSnippets == 1 ? "" : "s"), \(summary.importedGroups) group\(summary.importedGroups == 1 ? "" : "s")")
                            .fontWeight(.medium)
                    }
                    GridRow {
                        Text("Merged duplicates:")
                            .foregroundStyle(KzColor.inkMuted)
                        Text("\(summary.dedupeReport.mergedSnippets) snippet\(summary.dedupeReport.mergedSnippets == 1 ? "" : "s"), \(summary.dedupeReport.mergedGroups) group\(summary.dedupeReport.mergedGroups == 1 ? "" : "s")")
                    }
                    GridRow {
                        Text("Trigger conflicts:")
                            .foregroundStyle(KzColor.inkMuted)
                        if summary.dedupeReport.conflicts.isEmpty {
                            Text("None")
                                .foregroundStyle(KzColor.inkMuted)
                        } else {
                            Text("\(summary.dedupeReport.conflicts.count) (\(summary.dedupeReport.conflicts.map(\.trigger).joined(separator: ", ")))")
                                .foregroundStyle(KzColor.warning)
                        }
                    }
                    GridRow {
                        Text("Safety backup:")
                            .foregroundStyle(KzColor.inkMuted)
                        Text(summary.backupURL.lastPathComponent)
                            .kzTextStyle(.monoCaption)
                            .foregroundStyle(KzColor.inkMuted)
                    }
                }
                .kzTextStyle(.body)

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
                                    .kzTextStyle(.monoCaption)
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(.quaternary, in: Capsule())
                                Text(group.message)
                                    .kzTextStyle(.caption)
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
                                    .kzTextStyle(.monoCaption)
                                    .foregroundStyle(KzColor.inkMuted)
                                    .padding(.leading, 32)
                            }
                        }
                    }
                }
                .padding(.top, 6)
            }
            .frame(maxHeight: 140)
        } label: {
            HStack { KzIcon(.warning); Text(title) }
                .kzTextStyle(.body)
                .foregroundStyle(KzColor.warning)
        }
    }
}
