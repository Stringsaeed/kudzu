import AppKit
@testable import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct ListSnapshotTests {
    @Test func listHeader() throws {
        try snapshot("list-header", width: KzSize.list) {
            VStack(spacing: KzSpace.s4) {
                KzListHeader("All snippets", subtitle: "18 snippets") {
                    KzIconButton(.sort, label: "Sort", caret: true, menuTitle: "Sort by") { KzMenuItem("Name", isChecked: true) {} }
                    KzIconButton(.compose, label: "New snippet (⌘N)") {}
                }
                KzListHeader("Settings", subtitle: "Kudzu 1.0")
                KzListHeader("A group with a very long name that must truncate", subtitle: "3 snippets") {
                    KzIconButton(.compose, label: "New snippet (⌘N)") {}
                }
            }
            .background(KzColor.surface)
        }
    }

    @Test func snippetRows() throws {
        try snapshot("snippet-row", width: KzSize.list - 2 * KzSpace.s4 + 2 * KzSpace.s4) {
            VStack(spacing: 1) {
                KzSnippetRow(name: ":an", triggers: [":an"], preview: "Muhammed Saeed", group: "base", uses: 1, isFavorite: true)
                KzSnippetRow(name: ":date", triggers: [":date"], preview: "2 Nov 2026", group: "base", isFavorite: true, isSelected: true)
                KzSnippetRow(name: "Hovered", triggers: [":gh"], preview: "github.com/Stringsaeed", group: "base")
                    .kzPreviewInteraction(hovered: true)
                KzSnippetRow(name: "Focused", triggers: [":m"], preview: "muhammed@example.com", uses: 12)
                    .kzPreviewInteraction(focused: true)
                KzSnippetRow(name: "Kudzu test", triggers: [";kudzu", ";kz"], preview: "Kudzu is working. Expanded at 00:57 in Notes, with a much longer preview that has to truncate", group: "Getting started", uses: 128)
                KzSnippetRow(name: "No preview", triggers: [";sig"])
                KzSnippetRow(name: "A very long snippet name that goes on and on until it truncates", triggers: [":l"], preview: "short", group: "A long group", uses: 1204, isFavorite: true)
            }
            .padding(.horizontal, KzSpace.s2)
        }
    }

    @Test func listRows() throws {
        try snapshot("list-row", width: KzSize.list) {
            VStack(spacing: 1) {
                KzListRow(meta: "Last used 4 hours ago", aside: "1", isSelected: true, accessibilityName: ":m") { KzTriggerChip(":m") }
                KzListRow(meta: "Last used 4 hours ago", aside: "1", accessibilityName: ":an") { KzTriggerChip(":an") }
                KzListRow("kudzu-backup-2026-11-02", meta: "kudzu-backup-2026-11-02.json", aside: "18") { KzBadge("Manual") }
                KzListRow("Hovered", meta: "kudzu-backup-2026-11-01.json", aside: "17") { KzBadge("Automatic", tone: .vine) }
                    .kzPreviewInteraction(hovered: true)
                KzListRow("Title only")
                KzListRow("A backup whose file name is far too long to fit on one line of the list pane", meta: "kudzu-backup-2026-10-31-extremely-long-file-name.json", aside: "1,204") { KzBadge("Failed", tone: .danger) }
            }
            .padding(.horizontal, KzSpace.s2)
        }
    }

    @Test func rowsInSelectionList() async throws {
        // `List` can't be drawn by ImageRenderer, so host one in an offscreen window and draw the window's view.
        for dark in [false, true] {
            let size = CGSize(width: KzSize.list, height: 300)
            let content = KzSelectionListSample().frame(width: size.width, height: size.height)
                .background(KzColor.surface)
                .environment(\.colorScheme, dark ? .dark : .light)
            let host = NSHostingView(rootView: content)
            host.frame = NSRect(origin: .zero, size: size)
            let window = NSWindow(contentRect: host.frame, styleMask: [.titled], backing: .buffered, defer: false)
            window.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
            window.contentView = host
            host.layoutSubtreeIfNeeded()
            // Give the table a runloop turn to be created and configured.
            await settle()
            host.layoutSubtreeIfNeeded()
            let rep = try #require(host.bitmapImageRepForCachingDisplay(in: host.bounds))
            host.cacheDisplay(in: host.bounds, to: rep)
            let data = try #require(rep.representation(using: .png, properties: [:]))
            try FileManager.default.createDirectory(at: snapshotDirectory, withIntermediateDirectories: true)
            try data.write(to: snapshotDirectory.appendingPathComponent("list-selection-list-\(dark ? "dark" : "light")@2x.png"))
        }
    }
}

/// A real `List` with Kudzu rows, one of them selected.
struct KzSelectionListSample: View {
    @State private var selection: Int? = 2
    let rows = [(":an", "Muhammed Saeed"), (":date", "2 Nov 2026"), (":espanso", "Hi there!"), (":fn", "Muhammed Ahmed Saeed"), (":gh", "github.com/Stringsaeed")]

    var body: some View {
        List(selection: $selection) {
            ForEach(Array(rows.enumerated()), id: \.offset) { index, row in
                KzSnippetRow(name: row.0, triggers: [row.0], preview: row.1, group: "base", isSelected: selection == index)
                    .kzListRow()
                    .tag(index)
            }
        }
        .kzSelectionList()
    }
}
