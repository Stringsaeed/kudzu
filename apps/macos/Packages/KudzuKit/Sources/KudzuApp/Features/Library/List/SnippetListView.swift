// The snippet list (`snippet-list.tsx`). A native `List` owns selection, shift-extend, click and
// arrow keys. J/K, Home/End, Page Up/Down, type-to-filter and ⌘A are the keys the list does not
// provide. Rows drag out as `LibraryDrag` (their text leaves Kudzu; groups accept the ids).

import KudzuCore
import KudzuDesign
import SwiftUI

struct SnippetListView: View {
    @Environment(AppModel.self) private var model
    @Environment(LibraryActions.self) private var actions
    @Environment(LibraryFocus.self) private var focus
    @State private var sort: SnippetSort = .relevance
    @State private var anchor: UUID?
    @FocusState private var field: Field?

    private typealias Field = SnippetListNavigation.Focus

    var body: some View {
        let hits = currentHits
        let ids = hits.map(\.snippet.id)
        VStack(spacing: 0) {
            header(count: hits.count)
            filterField
            list(hits: hits, ids: ids)
        }
        .onChange(of: model.view) { _, _ in
            focus.rows = []
            anchor = nil
        }
        .onChange(of: ids) { _, ids in applyFallback(ids) }
        .onChange(of: focus.request?.serial) { _, _ in followFocus() }
        .onAppear { applyFallback(ids); followFocus() }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Snippets")
    }

    private func header(count: Int) -> some View {
        HStack(spacing: 8) {
            HStack(spacing: 6) {
                Text(LibraryRoutes.title(model.view, groups: model.library.groups))
                    .font(.system(size: 13, weight: .semibold))
                    .lineLimit(1)
                Text("\(count)")
                    .font(.system(size: 13).monospacedDigit())
                    .foregroundStyle(Color.kz.faint)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            Menu {
                Picker("Sort by", selection: $sort) {
                    ForEach(SnippetSort.allCases) { mode in Text(mode.label).tag(mode) }
                }
            } label: {
                Icon.arrowDownUp.icon(16)
            }
            .kzMenuTrigger(.ghost, size: .iconSm)
            .accessibilityLabel("Sort")
            .kzHelp("Sort")
            Button { actions.newSnippet() } label: { Icon.plus.icon(16) }
                .buttonStyle(.kz(.ghost, size: .iconSm))
                .foregroundStyle(Color.kz.mutedForeground)
                .accessibilityLabel("New snippet")
                .kzHelp("New snippet", shortcut: "mod+n")
        }
        .padding(.leading, 20)
        .padding(.trailing, 8)
        .frame(height: 48)
    }

    private var filterField: some View {
        HStack(spacing: 8) {
            Icon.search.icon(14).foregroundStyle(Color.kz.faint)
            TextField("Filter by name, trigger or text", text: filterText)
                .textFieldStyle(.plain)
                .font(.system(size: 13))
                .focused($field, equals: .filter)
                .accessibilityLabel("Filter snippets")
                .onSubmit { focus.focusEditor() }
                .onKeyPress(phases: .down) { press in filterKey(press) }
            if model.filter.isEmpty {
                Shortcut("mod+f")
            } else {
                Button { model.setFilter("") } label: { Icon.x.icon(14) }
                    .buttonStyle(.plain)
                    .foregroundStyle(Color.kz.faint)
                    .accessibilityLabel("Clear filter")
            }
        }
        .padding(.horizontal, 8)
        .frame(height: 32)
        .background { filterWell }
        .padding(.horizontal, 12)
        .padding(.bottom, 8)
    }

    private var filterWell: some View {
        let shape = RoundedRectangle(cornerRadius: Radius.lg)
        return ZStack {
            Color.clear.depth(.well, fill: Color.kz.foreground.opacity(0.03), in: shape)
            if field == .filter {
                shape.strokeBorder(Color.kz.ring, lineWidth: 2)
            }
        }
    }

    private func list(hits: [SearchHit], ids: [UUID]) -> some View {
        List(selection: selection(ids: ids)) {
            if hits.isEmpty {
                empty
                    .kzListRow()
            } else {
                ForEach(hits, id: \.snippet.id) { hit in
                    row(hit)
                        .tag(hit.snippet.id)
                        .draggable(dragItem(hit.snippet)) {
                            Text(dragLabel(hit.snippet))
                                .font(.system(size: 12))
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(Color.kz.popover, in: RoundedRectangle(cornerRadius: Radius.md))
                        }
                        .kzListRow(insets: EdgeInsets(top: 2, leading: 8, bottom: 2, trailing: 8))
                }
            }
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
        .focused($field, equals: .list)
        .focusable()
        .onKeyPress(phases: .down) { press in listKey(press, ids: ids) }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            if let bulk = BulkState(selection: Array(focus.rows), library: model.library) {
                BulkBar(bulk: bulk, groups: model.library.groups)
                    .padding(8)
                    .transition(KZTransition.rise)
            }
        }
        .accessibilityLabel("\(LibraryRoutes.title(model.view, groups: model.library.groups)) snippets")
    }

    private func row(_ hit: SearchHit) -> some View {
        let summary = LibraryRowSummary.text(hit.snippet, library: model.library)
        let groupOff = hit.snippet.groupID.map { !Groups.isGroupEnabled(model.library.groups, id: $0) } ?? false
        let id = hit.snippet.id
        let multi = focus.rows.count > 1
        return SnippetRowView(
            hit: hit,
            usage: model.usage[id]?.count ?? 0,
            summary: summary,
            groupOff: groupOff,
            inSelection: multi ? focus.rows.contains(id) : id == model.selectedId,
            selected: id == model.selectedId,
            active: field == .list || !multi
        )
    }

    private var empty: some View {
        let filter = model.filter.trimmingCharacters(in: .whitespacesAndNewlines)
        return VStack(spacing: 12) {
            if !filter.isEmpty {
                Text("No snippets match “\(filter)”")
                    .font(.system(size: 14))
                    .foregroundStyle(Color.kz.mutedForeground)
                HStack(spacing: 8) {
                    Button("Clear filter") { model.setFilter("") }
                        .buttonStyle(.kz(.secondary, size: .sm))
                    Button { actions.newSnippet(SnippetListLogic.draft(fromFilter: model.filter)) } label: {
                        ButtonLabel("Create it", icon: Icon.plus)
                    }
                    .buttonStyle(.kz(size: .sm))
                }
            } else {
                let copy = SnippetListLogic.emptyCopy(model.view)
                VStack(spacing: 4) {
                    Text(copy.title).font(.system(size: 14, weight: .medium))
                    Text(copy.body)
                        .font(.system(size: 12))
                        .foregroundStyle(Color.kz.mutedForeground)
                        .multilineTextAlignment(.center)
                }
                if copy.offersNew {
                    Button { actions.newSnippet() } label: { ButtonLabel("New snippet", icon: Icon.plus) }
                        .buttonStyle(.kz(size: .sm))
                }
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 24)
        .padding(.top, filter.isEmpty ? 64 : 48)
        .rise()
    }

    // MARK: Selection

    private var currentHits: [SearchHit] {
        let scoped = SnippetListLogic.scoped(model.library, route: model.view, usage: model.usage)
        return SnippetListLogic.hits(scoped, usage: model.usage, filter: model.filter, sort: sort, route: model.view)
    }

    private func selection(ids: [UUID]) -> Binding<Set<UUID>> {
        Binding(
            get: {
                if focus.rows.count > 1 { return focus.rows }
                return model.selectedId.map { [$0] } ?? []
            },
            set: { new in
                let old = focus.rows.count > 1 ? focus.rows : (model.selectedId.map { [$0] } ?? [])
                let primary = ListSelection.primary(old: old, new: new, previous: model.selectedId, ids: ids)
                let multi = new.count > 1 ? new : []
                if focus.rows != multi { focus.rows = multi }
                if new.count <= 1 { anchor = primary }
                if primary != model.selectedId { model.select(primary) }
            }
        )
    }

    private func move(by delta: Int, extend: Bool, ids: [UUID]) {
        let current = ListSelection(
            current: model.selectedId,
            rows: focus.rows.count > 1 ? focus.rows : (model.selectedId.map { [$0] } ?? []),
            anchor: anchor
        )
        let next = SnippetListNavigation(selection: current, focus: field ?? .list)
            .moved(in: ids, by: delta, extend: extend)
        anchor = next.selection.anchor
        focus.rows = next.selection.rows.count > 1 ? next.selection.rows : []
        model.select(next.selection.current)
        field = next.focus
    }

    private func applyFallback(_ ids: [UUID]) {
        let libraryIds = Set(model.library.snippets.map(\.id))
        guard let next = SnippetListLogic.fallbackSelection(ids: ids, selectedId: model.selectedId, filter: model.filter, route: model.view, libraryIds: libraryIds) else { return }
        model.select(next)
        focus.rows = []
        anchor = next
    }

    private func followFocus() {
        switch focus.request?.target {
        case .filter: field = .filter
        case .list: field = .list
        default: break
        }
    }

    private var filterText: Binding<String> {
        Binding(get: { model.filter }, set: { model.setFilter($0) })
    }

    private func filterKey(_ press: KeyPress) -> KeyPress.Result {
        let extend = press.modifiers.contains(.shift)
        switch press.key {
        case .downArrow: move(by: 1, extend: extend, ids: currentHits.map(\.snippet.id)); return .handled
        case .upArrow: move(by: -1, extend: extend, ids: currentHits.map(\.snippet.id)); return .handled
        case .return: focus.focusEditor(); return .handled
        case .escape:
            if !model.filter.isEmpty { model.setFilter(""); return .handled }
            field = .list
            return .handled
        default: return .ignored
        }
    }

    private func listKey(_ press: KeyPress, ids: [UUID]) -> KeyPress.Result {
        let mods = press.modifiers
        if mods.contains(.command), press.characters.lowercased() == "a" {
            focus.rows = Set(ids)
            return .handled
        }
        if mods.contains(.command) || mods.contains(.control) || mods.contains(.option) { return .ignored }
        let extend = mods.contains(.shift)
        switch press.key {
        case .home: move(by: -ids.count, extend: extend, ids: ids); return .handled
        case .end: move(by: ids.count, extend: extend, ids: ids); return .handled
        case .pageUp: move(by: -8, extend: extend, ids: ids); return .handled
        case .pageDown: move(by: 8, extend: extend, ids: ids); return .handled
        case .return, .rightArrow: focus.focusEditor(); return .handled
        case .upArrow: move(by: -1, extend: extend, ids: ids); return .handled
        case .downArrow: move(by: 1, extend: extend, ids: ids); return .handled
        case .escape:
            if focus.rows.count > 1 { focus.rows = []; return .handled }
            return .ignored
        default: break
        }
        if press.characters == "/" { field = .filter; return .handled }
        if press.characters == "j" || press.characters == "k" {
            move(by: press.characters == "j" ? 1 : -1, extend: extend, ids: ids)
            return .handled
        }
        if press.characters.count == 1, press.characters.first?.isWhitespace == false {
            model.setFilter(model.filter + press.characters)
            field = .filter
            return .handled
        }
        return .ignored
    }

    private func dragItem(_ snippet: Snippet) -> LibraryDrag {
        let ids = focus.rows.contains(snippet.id) && focus.rows.count > 1 ? Array(focus.rows) : [snippet.id]
        return LibraryDrag.snippets(ids, in: model.library)
    }

    private func dragLabel(_ snippet: Snippet) -> String {
        if focus.rows.contains(snippet.id), focus.rows.count > 1 { return "\(focus.rows.count) snippets" }
        return snippet.name.isEmpty ? "Snippet" : snippet.name
    }
}

/// The bulk bar (`bulk` in `snippet-list.tsx`), shown only for a multi-selection.
struct BulkBar: View {
    var bulk: BulkState
    var groups: [SnippetGroup]
    @Environment(LibraryActions.self) private var actions
    @Environment(LibraryFocus.self) private var focus

    var body: some View {
        HStack(spacing: 4) {
            Text("\(bulk.ids.count) selected")
                .font(.system(size: 12, weight: .medium).monospacedDigit())
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.leading, 8)
            Button { actions.perform { try await actions.model.favoriteAll(bulk.ids) } } label: { Icon.star.icon(16) }
                .buttonStyle(.kz(.ghost, size: .iconSm))
                .disabled(!bulk.canFavorite)
                .accessibilityLabel("Favorite all")
                .kzHelp("Add to favorites")
            Button { actions.perform { try await actions.model.toggleEnabledEach(bulk.ids) } } label: { Icon.circleOff.icon(16) }
                .buttonStyle(.kz(.ghost, size: .iconSm))
                .accessibilityLabel(bulk.allEnabled ? "Turn off all" : "Turn on all")
                .kzHelp("Toggle on/off")
            Menu {
                Section("Move \(bulk.ids.count) to") {
                    Button("No group") { actions.move(bulk.ids, to: nil) }
                    ForEach(Groups.flatten(Groups.buildTree(groups)), id: \.group.id) { node in
                        Button(Groups.groupPath(groups, id: node.group.id).joined(separator: " › ")) {
                            actions.move(bulk.ids, to: node.group.id)
                        }
                    }
                }
            } label: {
                Icon.folderInput.icon(16)
            }
            .kzMenuTrigger(.ghost, size: .iconSm)
            .accessibilityLabel("Move to group")
            Button {
                actions.requestDelete(bulk.ids)
                focus.rows = []
            } label: { Icon.trash2.icon(16) }
                .buttonStyle(.kz(.ghost, size: .iconSm))
                .foregroundStyle(Color.kz.destructive)
                .accessibilityLabel("Delete selected")
        }
        .padding(6)
        .depth(.float, fill: Color.kz.surfaceRaised, in: RoundedRectangle(cornerRadius: Radius.xl))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Selection actions")
    }
}
