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

@MainActor
@Suite struct ListContextMenuTests {
    /// Right-clicking a row of a real `List` targets that row's menu, as NSTableView's clicked row
    /// does, and the row draws its outline while the menu is open.
    @Test func rightClickTargetsTheRowUnderThePointer() async throws {
        let size = CGSize(width: KzSize.list, height: 400)
        let model = KzContextListModel()
        let host = NSHostingView(rootView: KzContextListSample(model: model).frame(width: size.width, height: size.height))
        host.frame = NSRect(origin: .zero, size: size)
        let window = NSWindow(contentRect: host.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = host
        host.layoutSubtreeIfNeeded()
        await settle()
        host.layoutSubtreeIfNeeded()

        let table = try #require(Self.firstTable(in: host))
        for row in [0, 3, 1] {
            let rowRect = table.rect(ofRow: row)
            let point = table.convert(CGPoint(x: rowRect.midX, y: rowRect.midY), to: nil)
            let region = KzContextMenuRegistry.shared.region(at: point, in: window)
            #expect(region?.makeMenu?().title == "Row \(row)")
            // The row's side gutter (outside the row's content) still belongs to the row, as in NSTableView.
            let gutter = table.convert(CGPoint(x: rowRect.minX + 2, y: rowRect.midY), to: nil)
            #expect(KzContextMenuRegistry.shared.region(at: gutter, in: window)?.makeMenu?().title == "Row \(row)")
        }
        // Below the last row: no row, so no row menu.
        let below = table.convert(CGPoint(x: size.width / 2, y: table.rect(ofRow: table.numberOfRows - 1).maxY + 4), to: nil)
        #expect(KzContextMenuRegistry.shared.region(at: below, in: window) == nil)

        // Opening the menu marks the row as the target; the selection doesn't change (as in Finder).
        model.target = 3
        await settle(0.2)
        host.layoutSubtreeIfNeeded()
        let rep = try #require(host.bitmapImageRepForCachingDisplay(in: host.bounds))
        host.cacheDisplay(in: host.bounds, to: rep)
        let data = try #require(rep.representation(using: .png, properties: [:]))
        try FileManager.default.createDirectory(at: snapshotDirectory, withIntermediateDirectories: true)
        try data.write(to: snapshotDirectory.appendingPathComponent("list-context-target@2x.png"))
        #expect(model.selection == 1)
    }

    /// Side-by-side regions outside a list: each click goes to the view under it, not a neighbour
    /// whose anchor's visible rect (views don't clip to bounds) reaches over.
    @Test func siblingRegionsDontOverlap() async throws {
        let size = CGSize(width: 300, height: 120)
        let content = HStack(spacing: 20) {
            ForEach(0..<3) { index in
                Color.gray.frame(width: 60, height: 40).kzContextMenu(title: "Box \(index)") { KzMenuItem("Open") {} }
            }
        }
        .frame(width: size.width, height: size.height)
        let host = NSHostingView(rootView: content)
        host.frame = NSRect(origin: .zero, size: size)
        let window = NSWindow(contentRect: host.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = host
        host.layoutSubtreeIfNeeded()
        await settle(0.2)
        // The boxes are at x 40…100, 120…180 and 200…260, y 40…80.
        let registry = KzContextMenuRegistry.shared
        #expect(registry.region(at: CGPoint(x: 70, y: 60), in: window)?.makeMenu?().title == "Box 0")
        #expect(registry.region(at: CGPoint(x: 150, y: 60), in: window)?.makeMenu?().title == "Box 1")
        #expect(registry.region(at: CGPoint(x: 230, y: 60), in: window)?.makeMenu?().title == "Box 2")
        #expect(registry.region(at: CGPoint(x: 110, y: 60), in: window) == nil) // between boxes
        #expect(registry.region(at: CGPoint(x: 150, y: 100), in: window) == nil) // above
    }

    private static func firstTable(in view: NSView) -> NSTableView? {
        if let table = view as? NSTableView { return table }
        for sub in view.subviews { if let found = firstTable(in: sub) { return found } }
        return nil
    }
}

@MainActor @Observable
final class KzContextListModel {
    var selection: Int? = 1
    var target: Int?
}

/// A real `List` whose rows each have a context menu and the right-clicked outline.
struct KzContextListSample: View {
    @Bindable var model: KzContextListModel
    let rows = [":an", ":date", ":espanso", ":fn", ":gh"]

    var body: some View {
        List(selection: $model.selection) {
            ForEach(Array(rows.enumerated()), id: \.offset) { index, name in
                KzSnippetRow(name: name, triggers: [name], isSelected: model.selection == index)
                    .kzContextMenu(title: "Row \(index)", isPresented: Binding(
                        get: { model.target == index },
                        set: { model.target = $0 ? index : (model.target == index ? nil : model.target) }
                    )) {
                        KzMenuItem("Duplicate") {}
                    }
                    .kzContextTarget(model.target == index)
                    .kzListRow()
                    .tag(index)
            }
        }
        .kzSelectionList()
        .background(KzColor.surface)
    }
}
