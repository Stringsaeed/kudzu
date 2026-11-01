import KudzuUI
import SwiftUI

struct SnippetRowPage: GalleryPage {
    static let id = "snippet-row"
    static let title = "SnippetRow"
    static let section = GallerySection.components
    static let icon = KzIconName.list

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Name and star, group and use count; trigger chips and a one-line preview. Selected is vine-soft; hover is surface-hover; no separators. A plain row: selection belongs to the list. Use List(selection:) with .kzListRow() and .kzSelectionList().") {
            SpecimenSection("States", note: "Selected, hovered, focused, long content.") {
                VStack(spacing: 1) {
                    KzSnippetRow(name: ":an", triggers: [":an"], preview: "Muhammed Saeed", group: "base", uses: 1, isFavorite: true)
                    KzSnippetRow(name: ":date", triggers: [":date"], preview: "2 Nov 2026", group: "base", isFavorite: true, isSelected: true)
                    KzSnippetRow(name: "Hovered", triggers: [":gh"], preview: "github.com/Stringsaeed", group: "base").kzPreviewInteraction(hovered: true)
                    KzSnippetRow(name: "Focused", triggers: [":m"], preview: "muhammed@example.com", uses: 12).kzPreviewInteraction(focused: true)
                    KzSnippetRow(name: "Kudzu test", triggers: [";kudzu", ";kz"], preview: "Kudzu is working. Expanded at 00:57 in Notes, with a long preview that truncates", group: "Getting started", uses: 128)
                    KzSnippetRow(name: "No preview", triggers: [";sig"])
                }
                .padding(KzSpace.s2)
                .frame(width: KzSize.list)
                .background(KzColor.surface)
            }
            SpecimenSection("In a List", note: "A real List(selection:): arrow keys, type-to-select and recycling come from AppKit. Use ↑ ↓ after clicking a row.") {
                SnippetListDemo()
            }
        }
    }
}

/// A live `List(selection:)` of a few hundred rows; offscreen snapshots show a static column instead.
private struct SnippetListDemo: View {
    @State private var selection: Int? = 1
    @Environment(\.kzSnapshotRendering) private var snapshotRendering
    private let rows = (0..<500).map { (name: ":snippet\($0)", preview: "Expansion number \($0)") }

    var body: some View {
        Group {
            if snapshotRendering {
                VStack(spacing: 1) {
                    ForEach(0..<3, id: \.self) { index in
                        KzSnippetRow(name: rows[index].name, triggers: [rows[index].name], preview: rows[index].preview, group: "base", isSelected: index == 1)
                    }
                }.padding(.horizontal, KzSpace.s2)
            } else {
                List(selection: $selection) {
                    ForEach(rows.indices, id: \.self) { index in
                        KzSnippetRow(name: rows[index].name, triggers: [rows[index].name], preview: rows[index].preview, group: "base", isSelected: selection == index)
                            .kzListRow()
                            .tag(index)
                    }
                }
                .kzSelectionList()
            }
        }
        .frame(width: KzSize.list, height: 260)
        .background(KzColor.surface)
    }
}
