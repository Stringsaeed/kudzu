import KudzuCore
import SwiftUI

/// Middle column: the snippets in the selected scope, with search, sort, multi-select and context menu.
struct SnippetListView: View {
    @Environment(LibraryModel.self) private var library
    @Environment(BrowserState.self) private var browser
    @AppStorage("snippetSortOrder") private var sortRaw = SnippetSortOrder.name.rawValue

    @State private var deleteRequest: [UUID] = []
    @State private var showDeleteConfirmation = false

    private var sort: SnippetSortOrder { SnippetSortOrder(rawValue: sortRaw) ?? .name }

    var body: some View {
        @Bindable var browser = browser
        let rows = library.snippets(in: browser.scope, query: browser.query, sort: sort)
        List(rows, selection: $browser.selection) { snippet in
            SnippetRow(snippet: snippet, showsGroup: showsGroup(for: snippet))
                .onDrag {
                    let ids = browser.selection.contains(snippet.id) ? Array(browser.selection) : [snippet.id]
                    browser.dragging = .snippets(ids)
                    return .kudzuDragMarker()
                }
        }
        .contextMenu(forSelectionType: UUID.self) { ids in
            menu(for: Array(ids))
        } primaryAction: { _ in }
        .onDeleteCommand { requestDelete(Array(browser.selection)) }
        .overlay { emptyState(rowCount: rows.count) }
        .searchable(text: $browser.query, placement: .toolbar, prompt: "Search snippets")
        .navigationTitle(title)
        .navigationSubtitle(subtitle(count: rows.count))
        .toolbar {
            ToolbarItem {
                Menu {
                    Picker("Sort By", selection: $sortRaw) {
                        ForEach(SnippetSortOrder.allCases) { order in
                            Text(order.title).tag(order.rawValue)
                        }
                    }
                    .pickerStyle(.inline)
                } label: {
                    Label("Sort", systemImage: "arrow.up.arrow.down")
                }
                .help("Sort snippets")
                .disabled(browser.scope == .recent)
            }
        }
        .confirmationDialog(
            deleteTitle,
            isPresented: $showDeleteConfirmation,
            titleVisibility: .visible
        ) {
            Button("Delete", role: .destructive) {
                library.deleteSnippets(deleteRequest)
                browser.prune(using: library)
                deleteRequest = []
            }
            Button("Cancel", role: .cancel) { deleteRequest = [] }
        } message: {
            Text("This also removes their usage history. You can't undo this.")
        }
    }

    // MARK: - Titles

    private var title: String {
        switch browser.scope {
        case .all: "All Snippets"
        case .favorites: "Favorites"
        case .recent: "Recently Used"
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

    // MARK: - Deleting

    private var deleteTitle: String {
        if deleteRequest.count == 1, let snippet = library.snippetsByID[deleteRequest[0]] {
            return "Delete “\(snippet.name)”?"
        }
        return "Delete \(deleteRequest.count) snippets?"
    }

    private func requestDelete(_ ids: [UUID]) {
        let existing = ids.filter { library.snippetsByID[$0] != nil }
        guard !existing.isEmpty else { return }
        deleteRequest = existing
        showDeleteConfirmation = true
    }

    // MARK: - Context menu

    @ViewBuilder
    private func menu(for ids: [UUID]) -> some View {
        if ids.isEmpty {
            Button("New Snippet") { browser.newSnippet(library: library) }
        } else {
            let snippets = ids.compactMap { library.snippetsByID[$0] }
            let allFavorite = snippets.allSatisfy(\.isFavorite)
            let allEnabled = snippets.allSatisfy(\.isEnabled)

            Button("Duplicate") {
                let copies = library.duplicate(ids)
                if !copies.isEmpty { browser.selection = Set(copies) }
            }
            Button(allFavorite ? "Remove from Favorites" : "Add to Favorites") {
                library.update(ids) { $0.isFavorite = !allFavorite }
            }
            Button(allEnabled ? "Disable" : "Enable") {
                library.update(ids) { $0.isEnabled = !allEnabled }
            }
            moveMenu(for: ids, snippets: snippets)
            Divider()
            Button("Delete…", role: .destructive) { requestDelete(ids) }
        }
    }

    private func moveMenu(for ids: [UUID], snippets: [SnippetRecord]) -> some View {
        let currentGroups = Set(snippets.map(\.groupID))
        return Menu("Move to Group") {
            Button {
                library.moveSnippets(ids, to: nil)
            } label: {
                if currentGroups == [nil] { Label("No Group", systemImage: "checkmark") } else { Text("No Group") }
            }
            if !library.tree.allGroups.isEmpty { Divider() }
            ForEach(library.tree.allGroups) { group in
                Button {
                    library.moveSnippets(ids, to: group.id)
                } label: {
                    let name = library.tree.path(to: group.id).joined(separator: " / ")
                    if currentGroups == [group.id] { Label(name, systemImage: "checkmark") } else { Text(name) }
                }
            }
        }
    }

    // MARK: - Empty states

    @ViewBuilder
    private func emptyState(rowCount: Int) -> some View {
        if rowCount == 0, library.isLoaded {
            if !browser.query.isEmpty {
                ContentUnavailableView.search(text: browser.query)
            } else {
                switch browser.scope {
                case .all:
                    ContentUnavailableView {
                        Label("No Snippets Yet", systemImage: "text.badge.plus")
                    } description: {
                        Text("Create a snippet and type its trigger anywhere to expand it.")
                    } actions: {
                        Button("New Snippet") { browser.newSnippet(library: library) }
                            .buttonStyle(.borderedProminent)
                    }
                case .favorites:
                    ContentUnavailableView("No Favorites", systemImage: "star",
                                           description: Text("Mark a snippet as a favorite to find it here."))
                case .recent:
                    ContentUnavailableView("Nothing Used Yet", systemImage: "clock",
                                           description: Text("Snippets you expand show up here."))
                case .group:
                    ContentUnavailableView {
                        Label("This Group Is Empty", systemImage: "folder")
                    } description: {
                        Text("Create a snippet here, or drag snippets onto the group in the sidebar.")
                    } actions: {
                        Button("New Snippet") { browser.newSnippet(library: library) }
                    }
                case .stats:
                    EmptyView()
                }
            }
        }
    }
}

/// One snippet in the list: name, trigger capsules, use count.
private struct SnippetRow: View {
    @Environment(LibraryModel.self) private var library
    let snippet: SnippetRecord
    let showsGroup: Bool

    var body: some View {
        let count = library.stats[snippet.id]?.count ?? 0
        let isActive = library.isActive(snippet)
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                Text(snippet.name.isEmpty ? "Untitled" : snippet.name)
                    .fontWeight(.medium)
                    .lineLimit(1)
                if snippet.isFavorite {
                    Image(systemName: "star.fill")
                        .font(.caption2)
                        .foregroundStyle(.yellow)
                        .accessibilityLabel("Favorite")
                }
                Spacer(minLength: 8)
                if count > 0 {
                    Text(count.formatted())
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                        .help("Used \(count) time\(count == 1 ? "" : "s")")
                }
            }
            HStack(spacing: 4) {
                ForEach(snippet.triggers.prefix(3), id: \.self) { trigger in
                    Text(trigger)
                        .font(.system(.caption, design: .monospaced))
                        .lineLimit(1)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 1)
                        .background(Color.primary.opacity(0.12), in: Capsule())
                }
                if snippet.triggers.count > 3 {
                    Text("+\(snippet.triggers.count - 3)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                if showsGroup, let path = library.groupPath(of: snippet) {
                    Text(path)
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                        .truncationMode(.head)
                }
            }
        }
        .padding(.vertical, 2)
        .opacity(isActive ? 1 : 0.5)
        .accessibilityElement(children: .combine)
    }
}
