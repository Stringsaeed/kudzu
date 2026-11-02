import KudzuCore
import KudzuUI
import SwiftUI

/// The list pane: header, search, and the snippets in the selected scope, with sort, multi-select,
/// drag into groups and a right-click menu.
struct SnippetListView: View {
    @Environment(LibraryModel.self) private var library
    @Environment(BrowserState.self) private var browser
    @Environment(\.kzSnapshotRendering) private var snapshotRendering
    @AppStorage("snippetSortOrder") private var sortRaw = SnippetSortOrder.name.rawValue

    let searchFocus: FocusState<Bool>.Binding
    @State private var contextTarget: UUID?

    private var sort: SnippetSortOrder { SnippetSortOrder(rawValue: sortRaw) ?? .name }

    var body: some View {
        @Bindable var browser = browser
        let rows = library.snippets(in: browser.scope, query: browser.query, sort: sort)
        KzListHeader(title, subtitle: subtitle(count: rows.count)) {
            KzIconButton(.sort, label: "Sort", caret: true, menuTitle: "Sort by") {
                for order in SnippetSortOrder.allCases {
                    KzMenuItem(order.sentenceTitle, isChecked: sort == order) { sortRaw = order.rawValue }
                }
            }
            // Recently used is always newest first.
            .disabled(browser.scope == .recent)
            KzIconButton(.compose, label: "New snippet (⌘N)") { browser.newSnippet(library: library) }
        }
        KzSearchField(text: $browser.query, focus: searchFocus)
            .padding(.horizontal, KzSpace.s3)
            .padding(.bottom, KzSpace.s2)
        list(rows)
            .overlay { emptyState(rowCount: rows.count) }
    }

    @ViewBuilder
    private func list(_ rows: [SnippetRecord]) -> some View {
        @Bindable var browser = browser
        if snapshotRendering {
            // `NSTableView` can't render offscreen: snapshots show the same rows in a plain column.
            VStack(spacing: 1) {
                ForEach(rows) { row($0) }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, KzSpace.s2)
            .frame(maxHeight: .infinity, alignment: .top)
        } else {
            List(selection: $browser.selection) {
                ForEach(rows) { snippet in
                    row(snippet)
                        .onDrag {
                            browser.dragging = .snippets(browser.targets(for: snippet.id))
                            return .kudzuDragMarker()
                        }
                        .kzListRow()
                        .tag(snippet.id)
                }
            }
            .kzSelectionList()
            .onDeleteCommand { browser.requestDelete(Array(browser.selection), in: library) }
            // Right-clicking the empty space below the rows.
            .kzContextMenu {
                KzMenuItem("New snippet", icon: .compose) { browser.newSnippet(library: library) }
                    .keyboardShortcut("n")
            }
        }
    }

    private func row(_ snippet: SnippetRecord) -> some View {
        let isTarget = contextTarget.map { browser.targets(for: $0).contains(snippet.id) } ?? false
        return KzSnippetRow(
            name: snippet.name.isEmpty ? "Untitled" : snippet.name,
            triggers: snippet.triggers,
            preview: SnippetPreviewText.line(for: snippet, contents: library.contentByID),
            group: showsGroup(for: snippet) ? library.groupPath(of: snippet) : nil,
            uses: library.stats[snippet.id]?.count,
            isFavorite: snippet.isFavorite,
            isSelected: browser.selection.contains(snippet.id)
        )
        // Disabled snippets, and snippets in a disabled group, never expand.
        .opacity(library.isActive(snippet) ? 1 : 0.5)
        .kzContextMenu(isPresented: contextBinding(snippet.id)) {
            SnippetMenus.actions(for: browser.targets(for: snippet.id), library: library, browser: browser)
        }
        .kzContextTarget(isTarget)
    }

    private func contextBinding(_ id: UUID) -> Binding<Bool> {
        Binding(
            get: { contextTarget == id },
            set: { open in
                if open { contextTarget = id } else if contextTarget == id { contextTarget = nil }
            }
        )
    }

    // MARK: - Titles

    private var title: String {
        switch browser.scope {
        case .all: "All snippets"
        case .favorites: "Favorites"
        case .recent: "Recently used"
        case .stats: "Stats"
        case .group(let id): library.tree.group(id)?.name ?? "Group"
        }
    }

    private func subtitle(count: Int) -> String {
        "\(count) snippet\(count == 1 ? "" : "s")"
    }

    /// Group names help when the list mixes groups; inside a single leaf group they'd be noise.
    private func showsGroup(for snippet: SnippetRecord) -> Bool {
        guard snippet.groupID != nil else { return false }
        if case .group(let id) = browser.scope { return snippet.groupID != id }
        return true
    }

    // MARK: - Empty states

    @ViewBuilder
    private func emptyState(rowCount: Int) -> some View {
        if rowCount == 0, library.isLoaded {
            Group {
                if !browser.query.isEmpty {
                    KzEmptyState("No results", icon: .search, message: "No snippets match “\(browser.query)”. Check the spelling or try a trigger.")
                } else {
                    switch browser.scope {
                    case .all:
                        KzEmptyState("No snippets yet", message: "Create a snippet and type its trigger anywhere to expand it.") {
                            KzButton("New snippet", variant: .primary, icon: .compose) { browser.newSnippet(library: library) }
                                .padding(.top, KzSpace.s2)
                        }
                    case .favorites:
                        KzEmptyState("No favorites", icon: .star, message: "Mark a snippet as a favorite to find it here.")
                    case .recent:
                        KzEmptyState("Nothing used yet", icon: .clock, message: "Snippets you expand show up here.")
                    case .group:
                        KzEmptyState("This group is empty", icon: .folder,
                                     message: "Create a snippet here, or drag snippets onto the group in the sidebar.") {
                            KzButton("New snippet", icon: .compose) { browser.newSnippet(library: library) }
                                .padding(.top, KzSpace.s2)
                        }
                    case .stats:
                        EmptyView()
                    }
                }
            }
            .frame(maxHeight: .infinity, alignment: .top)
            .padding(.top, KzSpace.s8)
        }
    }
}

extension SnippetSortOrder {
    /// The menu title in sentence case.
    var sentenceTitle: String {
        switch self {
        case .name: "Name"
        case .recentlyUsed: "Recently used"
        case .mostUsed: "Most used"
        case .recentlyEdited: "Recently edited"
        }
    }
}

/// Snippet commands shared by the list's right-click menu and the detail toolbar's More menu.
@MainActor
enum SnippetMenus {
    /// Duplicate, favorite, enable, move to group and delete, for one snippet or several.
    @KzMenuBuilder
    static func actions(for ids: [UUID], library: LibraryModel, browser: BrowserState) -> [KzMenuElement] {
        let snippets = ids.compactMap { library.snippetsByID[$0] }
        let allFavorite = !snippets.isEmpty && snippets.allSatisfy(\.isFavorite)
        let allEnabled = !snippets.isEmpty && snippets.allSatisfy(\.isEnabled)

        KzMenuItem("Duplicate", icon: .copy) {
            let copies = library.duplicate(ids)
            if !copies.isEmpty { browser.selection = Set(copies) }
        }
        .keyboardShortcut("d")
        KzMenuItem("Favorite", icon: .star, isChecked: allFavorite) {
            library.update(ids) { $0.isFavorite = !allFavorite }
        }
        KzMenuItem("Enabled", icon: allEnabled ? .pause : .play, isChecked: allEnabled) {
            library.update(ids) { $0.isEnabled = !allEnabled }
        }
        moveMenu(for: ids, snippets: snippets, library: library)
        KzMenuSeparator()
        KzMenuItem(ids.count > 1 ? "Delete \(ids.count) snippets…" : "Delete…", icon: .trash, role: .danger) {
            browser.requestDelete(ids, in: library)
        }
        .keyboardShortcut(.delete, modifiers: [])
    }

    private static func moveMenu(for ids: [UUID], snippets: [SnippetRecord], library: LibraryModel) -> KzSubmenu {
        let currentGroups = Set(snippets.map(\.groupID))
        let groups = library.tree.allGroups
        return KzSubmenu("Move to group", icon: .folder) {
            KzMenuItem("No group", isChecked: currentGroups == [nil]) { library.moveSnippets(ids, to: nil) }
            if !groups.isEmpty { KzMenuSeparator() }
            for group in groups {
                KzMenuItem(library.tree.path(to: group.id).joined(separator: " / "), isChecked: currentGroups == [group.id]) {
                    library.moveSnippets(ids, to: group.id)
                }
            }
        }
    }
}
