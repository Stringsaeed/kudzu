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
    @State private var listAnchor = ViewAnchor()

    private var sort: SnippetSortOrder { SnippetSortOrder(rawValue: sortRaw) ?? .name }

    var body: some View {
        @Bindable var browser = browser
        let rows = library.snippets(in: browser.scope, query: browser.query, sort: sort)
        // Read so a refused selection change redraws the list on the selection it kept.
        let _ = browser.selectionEpoch
        HStack(spacing: 8) {
            Text(title).font(WorkspaceTheme.label).lineLimit(1)
            Text(rows.count.formatted()).font(WorkspaceTheme.caption).foregroundStyle(WorkspaceTheme.secondary)
            Spacer(minLength: 0)
            WorkspaceMenu {
                ForEach(SnippetSortOrder.allCases, id: \.self) { order in
                    Button { sortRaw = order.rawValue } label: {
                        if sort == order { Label(order.sentenceTitle, systemImage: "checkmark") }
                        else { Text(order.sentenceTitle) }
                    }
                }
            } label: { Image(systemName: "arrow.up.arrow.down").frame(width: 24, height: 28) }
            .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
            .tint(WorkspaceTheme.secondary)
            .help("Sort snippets").accessibilityLabel("Sort snippets")
            .disabled(browser.scope == .recent)
            WorkspaceIconButton("square.and.pencil", "New snippet (⌘N)") { browser.newSnippet(library: library) }
        }
        .padding(.horizontal, 14).frame(height: 52)
        .foregroundStyle(WorkspaceTheme.ink)
        .kzWindowDragArea()
        KzSearchField(text: $browser.query, shortcut: nil, focus: searchFocus,
                      selectAllToken: browser.searchSelectAllToken, unboxed: true)
            .padding(.horizontal, 12).padding(.bottom, 10)
        list(rows)
            .overlay { emptyState(rowCount: rows.count) }
            .onAppear(perform: consumePaneRequest)
            .onChange(of: browser.paneRequest) { _, _ in consumePaneRequest() }
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
            .frame(minHeight: 0, maxHeight: .infinity, alignment: .top)
            .clipped()
        } else {
            List(selection: selectionBinding) {
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
            .accessibilityIdentifier("kudzu.results")
            .background { ResultsTableTag() }
            .viewAnchor(listAnchor)
            // Right-clicking the empty space below the rows.
            .kzContextMenu {
                KzMenuItem("New snippet", icon: .compose) { browser.newSnippet(library: library) }
                    .keyboardShortcut("n")
            }
        }
    }

    private func row(_ snippet: SnippetRecord) -> some View {
        let isTarget = contextTarget.map { browser.targets(for: $0).contains(snippet.id) } ?? false
        let selected = browser.selection.contains(snippet.id)
        let preview = SnippetPreviewText.line(for: snippet, contents: library.contentByID)
        return VStack(alignment: .leading, spacing: 5) {
            HStack(spacing: 6) {
                Text(snippet.name.isEmpty ? "Untitled" : snippet.name)
                    .font(WorkspaceTheme.label).foregroundStyle(WorkspaceTheme.ink).lineLimit(1)
                if snippet.isFavorite {
                    Image(systemName: "star.fill").font(.system(size: 9)).foregroundStyle(WorkspaceTheme.secondary)
                        .accessibilityLabel("Favorite")
                }
                Spacer(minLength: 0)
                Text(snippet.triggers.first ?? "No trigger")
                    .font(WorkspaceTheme.mono)
                    .foregroundStyle(selected ? WorkspaceTheme.accent : WorkspaceTheme.secondary)
                    .lineLimit(1)
                    .frame(maxWidth: 96, alignment: .trailing)
            }
            Text(preview ?? "Empty snippet")
                .font(.system(size: 12)).foregroundStyle(WorkspaceTheme.secondary).lineLimit(1)
        }
        .padding(.horizontal, 12).padding(.vertical, 9)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(selected ? WorkspaceTheme.selection : .clear)
        .overlay(alignment: .leading) {
            if selected { WorkspaceTheme.accent.frame(width: 2).padding(.vertical, 10) }
        }
        .contentShape(Rectangle())
        .help(([snippet.name] + snippet.triggers + [library.groupPath(of: snippet) ?? "No group", snippet.content]).joined(separator: "\n"))
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? .isSelected : [])
        .accessibilityValue(snippet.triggers.joined(separator: ", "))
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

    /// Selection writes go through the leave guard. Reading `selectionEpoch` keeps a refused click visible as the old row.
    private var selectionBinding: Binding<Set<UUID>> {
        Binding(
            get: { browser.selection },
            set: { browser.replaceSelection($0) }
        )
    }

    /// The list owns sort order, so it chooses the row focus lands on.
    /// Rows are read here, not captured from an earlier render, so a cleared query is already in effect.
    private func consumePaneRequest() {
        let rows = library.snippets(in: browser.scope, query: browser.query, sort: sort)
        switch browser.paneRequest {
        case .search:
            browser.paneRequest = nil
            searchFocus.wrappedValue = true
        case .results:
            browser.paneRequest = nil
            enterResults(rows: rows, then: nil)
        case .editor(let target):
            browser.paneRequest = nil
            enterResults(rows: rows, then: target)
        case .sidebar, nil:
            break
        }
    }

    private func enterResults(rows: [SnippetRecord], then editor: EditorFocusTarget?) {
        let visible = rows.map(\.id)
        let next = KzSelectionReconciler.selectionForEnteringResults(current: browser.selection, visible: visible)
        if next != browser.selection { browser.replaceSelection(next) }
        if editor == nil {
            focusResultsTable()
        }
        guard let editor else { return }
        let focused = browser.selection.count == 1 || browser.deferredSelection?.count == 1
        guard focused else { return }
        library.pendingFocus = editor
    }

    private func focusResultsTable() {
        if focusResultsTableNow() { return }
        // Leaving settings or Stats inserts the table on the next layout pass.
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(50))
            focusResultsTableNow()
        }
    }

    @discardableResult
    private func focusResultsTableNow() -> Bool {
        guard let table = listAnchor.nearest(NSTableView.self), let window = table.window else { return false }
        if table.accessibilityIdentifier() != "kudzu.results" {
            table.setAccessibilityIdentifier("kudzu.results")
        }
        return window.makeFirstResponder(table)
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
            let copies = browser.duplicateSnippets(ids, in: library)
            if !copies.isEmpty { browser.replaceSelection(Set(copies)) }
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
