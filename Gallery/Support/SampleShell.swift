import KudzuUI
import SwiftUI

/// One snippet in the sample library.
struct SampleSnippet: Identifiable, Equatable {
    var id: String
    var name: String
    var triggers: [String]
    var preview: String
    var group: String
    var uses: Int?
    var isFavorite = false

    static let library: [SampleSnippet] = [
        SampleSnippet(id: "an", name: ":an", triggers: [":an"], preview: "Muhammed Saeed", group: "base", uses: 1, isFavorite: true),
        SampleSnippet(id: "date", name: ":date", triggers: [":date"], preview: "2 Nov 2026", group: "base", isFavorite: true),
        SampleSnippet(id: "espanso", name: ":espanso", triggers: [":espanso"], preview: "Hi there!", group: "base", isFavorite: true),
        SampleSnippet(id: "fn", name: ":fn", triggers: [":fn"], preview: "Muhammed Ahmed Saeed", group: "base"),
        SampleSnippet(id: "gh", name: ":gh", triggers: [":gh"], preview: "github.com/Stringsaeed", group: "base"),
        SampleSnippet(id: "kudzu", name: "Kudzu test", triggers: [";kudzu", ";kz"], preview: "Kudzu is working. Expanded at 00:57 in Notes.", group: "Getting started"),
        SampleSnippet(id: "sig", name: "Signature", triggers: [";sig"], preview: "Best, Muhammed", group: "Getting started"),
    ]
}

/// The one window with realistic content, embedded (no real window chrome) for the gallery.
///
/// Every control is the real component and the menus work: sort, the detail toolbar's Debug and More,
/// Insert tag, Insert-mode Select, and a right-click menu on each row.
struct GallerySampleShell: View {
    @State private var snippets = SampleSnippet.library
    @State private var selection: String?
    @State private var sort = "Name"
    @State private var search = ""
    @State private var contextTarget: String?
    @State private var triggers: [String] = []
    @State private var rules = Rules.inherit
    @State private var insertion = "Inherit"
    @State private var lastAction = ""
    @Environment(\.kzSnapshotRendering) private var snapshotRendering

    private enum Rules: Hashable { case inherit, custom }

    init(selection: String = "date") {
        _selection = State(initialValue: selection)
        _triggers = State(initialValue: SampleSnippet.library.first { $0.id == selection }?.triggers ?? [])
    }

    private var selected: SampleSnippet? { snippets.first { $0.id == selection } }

    var body: some View {
        KzAppShell(mode: .embedded) {
            KzSidebarSection {
                KzSidebarItem("All snippets", icon: .stack, count: 18, isActive: true) {}
                KzSidebarItem("Favorites", icon: .star, count: 3) {}
                KzSidebarItem("Recently used", icon: .clock) {}
                KzSidebarItem("Stats", icon: .chart) {}
            }
            KzSidebarSection("Groups", action: { KzIconButton(.plus, label: "New group", size: .sm) {} }) {
                KzSidebarItem("base", icon: .folder, count: 15) {}
                KzSidebarItem("Getting started", icon: .folder, count: 3) {}
            }
        } sidebarHeader: {
            KzIconButton(.sidebar, label: "Hide sidebar", size: .sm) {}
        } sidebarFooter: {
            KzSidebarItem("Settings", icon: .sliders, shortcut: "⌘,") {}
        } list: {
            KzListHeader("All snippets", subtitle: "18 snippets") {
                KzIconButton(.sort, label: "Sort", caret: true, menuTitle: "Sort by") {
                    GalleryMenus.sort(selection: sort) { sort = $0 }
                }
                KzIconButton(.compose, label: "New snippet (⌘N)") {}
            }
            KzSearchField(text: $search).padding(.horizontal, KzSpace.s3).padding(.bottom, KzSpace.s2)
            snippetList
        } detail: {
            if let snippet = selected {
                detail(for: snippet)
            } else {
                Spacer(minLength: 0)
                KzEmptyState("No snippet selected") {
                    KzMessage("Choose a snippet to edit it, or press \(kbd: "⌘N") to create one.")
                }
                .frame(maxWidth: .infinity)
                Spacer(minLength: 0)
            }
        }
    }

    // MARK: List

    @ViewBuilder private var snippetList: some View {
        if snapshotRendering {
            // `NSTableView` can't render offscreen: snapshots show the same rows in a plain column.
            VStack(spacing: 1) { rows }.padding(.horizontal, KzSpace.s2)
            Spacer(minLength: 0)
        } else {
            List(selection: $selection) { rows }
                .kzSelectionList()
        }
    }

    private var rows: some View {
        ForEach(snippets) { snippet in
            KzSnippetRow(name: snippet.name, triggers: snippet.triggers, preview: snippet.preview, group: snippet.group,
                         uses: snippet.uses, isFavorite: snippet.isFavorite, isSelected: selection == snippet.id)
                .kzContextMenu(isPresented: contextBinding(snippet.id)) {
                    GalleryMenus.row(isFavorite: snippet.isFavorite, run: { run($0, on: snippet.id) },
                                     moveTo: { move(snippet.id, to: $0) })
                }
                .kzContextTarget(contextTarget == snippet.id)
                .kzListRow()
                .tag(snippet.id)
        }
    }

    private func contextBinding(_ id: String) -> Binding<Bool> {
        Binding(get: { contextTarget == id }, set: { open in contextTarget = open ? id : (contextTarget == id ? nil : contextTarget) })
    }

    private func run(_ action: String, on id: String) {
        guard let index = snippets.firstIndex(where: { $0.id == id }) else { return }
        switch action {
        case "Favorite": snippets[index].isFavorite.toggle()
        case "Duplicate":
            var copy = snippets[index]
            copy.id += "-copy"
            copy.name += " copy"
            snippets.insert(copy, at: index + 1)
        case "Delete":
            snippets.remove(at: index)
            if selection == id { selection = nil }
        default: lastAction = action
        }
    }

    private func move(_ id: String, to group: String) {
        guard let index = snippets.firstIndex(where: { $0.id == id }) else { return }
        snippets[index].group = group
    }

    // MARK: Detail

    @ViewBuilder private func detail(for snippet: SampleSnippet) -> some View {
        header(for: snippet)
        VStack(alignment: .leading, spacing: KzSpace.s5) {
            Text(snippet.name).kzMonoTitle().foregroundStyle(KzColor.ink).lineLimit(1)
            KzField("Triggers", aside: "Space or return to add") {
                KzTriggerInput(triggers: $triggers)
            }
            contentField(for: snippet)
            KzField("Preview") {
                Text(snippet.preview)
                    .kzTextStyle(.content)
                    .foregroundStyle(KzColor.ink)
                    .padding(.horizontal, 14).padding(.vertical, KzSpace.s3)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(KzColor.surfaceSunken, in: RoundedRectangle(cornerRadius: KzRadius.lg))
            }
            rulesDisclosure
            KzDisclosure("Advanced", summary: "Word boundary, case, undo") { EmptyView() }
            Spacer(minLength: 0)
            Rectangle().fill(KzColor.line).frame(height: 1)
            footer(for: snippet)
        }
        .padding(.horizontal, KzSpace.s6)
        .padding(.top, KzSpace.s2)
        .padding(.bottom, KzSpace.s6)
        .id(snippet.id)
        .onAppear { triggers = snippet.triggers }
        .onChange(of: selection) { _, new in triggers = snippets.first { $0.id == new }?.triggers ?? [] }
    }

    private func header(for snippet: SampleSnippet) -> some View {
        KzPaneHeader {
            HStack(spacing: 6) {
                KzIcon(.folder, size: 12)
                Text(snippet.group).kzTextStyle(.caption)
            }
            .foregroundStyle(KzColor.inkMuted)
            Spacer(minLength: 0)
            KzToolbarGroup(label: "Snippet actions") {
                KzIconButton(snippet.isFavorite ? .starFill : .star, label: "Favorite", tone: snippet.isFavorite ? .bloom : .neutral) {
                    run("Favorite", on: snippet.id)
                }
                KzIconButton(.bug, label: "Debug", caret: true, menuTitle: "Debug") {
                    GalleryMenus.debug { lastAction = $0 }
                }
                KzToolbarSeparator()
                KzIconButton(.more, label: "More", placement: .pullDown(.trailing)) {
                    GalleryMenus.snippetActions { run($0, on: snippet.id) }
                }
            }
        }
    }

    private func contentField(for snippet: SampleSnippet) -> some View {
        KzField("Content") {
            KzEditorFrame(minHeight: 70, insert: {
                KzButton("Insert", variant: .ghost, size: .sm, icon: .plus, iconEnd: .chevronDown, menuTitle: "Insert tag") {
                    GalleryMenus.insertTag { lastAction = "Inserted \($0)" }
                }
            }) {
                KzEditorTextView(content(for: snippet), isEditable: false)
            }
        }
    }

    private var rulesDisclosure: some View {
        KzDisclosure("Rules", isInitiallyExpanded: true) {
            HStack(spacing: KzSpace.s3) {
                KzSegmented("Rules", selection: $rules, options: [.init(.inherit, label: "Inherit from group"), .init(.custom, label: "Custom")])
                HStack(spacing: 6) {
                    KzIcon(.check, size: 14)
                    Text("Works in every app").kzTextStyle(.caption)
                }
                .foregroundStyle(KzColor.inkMuted)
            }
            HStack(spacing: KzSpace.s3) {
                Text("Insertion").kzTextStyle(.bodyStrong).foregroundStyle(KzColor.ink).frame(width: 72, alignment: .leading)
                KzSelect("Insertion", selection: $insertion, options: [("Inherit", "Inherit"), ("Type", "Type"), ("Paste", "Paste")])
            }
        }
    }

    private func footer(for snippet: SampleSnippet) -> some View {
        let count = snippet.uses ?? 0
        return Text("Created 1 Nov · Edited 2 Nov · \(count) \(count == 1 ? "expansion" : "expansions")")
            .kzTextStyle(.caption)
            .foregroundStyle(KzColor.inkMuted)
    }

    private func content(for snippet: SampleSnippet) -> [KzEditorSegment] {
        snippet.id == "date" ? [.tag("{date:%d %b %Y}")] : [.text(snippet.preview)]
    }
}
