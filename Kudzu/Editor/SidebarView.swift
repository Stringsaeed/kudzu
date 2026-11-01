import KudzuCore
import SwiftUI
import UniformTypeIdentifiers

/// Where a drag would land relative to a group row.
enum DropZone: Equatable {
    case before, into, after
}

struct DropIndicator: Equatable {
    var groupID: UUID?
    var zone: DropZone
}

/// Fixed library views plus the nested group tree.
struct SidebarView: View {
    @Environment(LibraryModel.self) private var library
    @Environment(BrowserState.self) private var browser

    @SceneStorage("sidebar.collapsedGroups") private var collapsedRaw = ""
    @State private var renaming: UUID?
    @State private var renameText = ""
    @State private var deleting: GroupRecord?
    @State private var indicator: DropIndicator?
    @FocusState private var renameFocused: Bool

    var body: some View {
        List(selection: scopeSelection) {
            Section {
                fixedRow("All Snippets", systemImage: "tray.full", scope: .all, count: library.snippets.count)
                fixedRow("Favorites", systemImage: "star", scope: .favorites,
                         count: library.snippets.lazy.filter(\.isFavorite).count)
                fixedRow("Recently Used", systemImage: "clock", scope: .recent, count: 0)
                fixedRow("Stats", systemImage: "chart.bar", scope: .stats, count: 0)
            }

            Section {
                ForEach(library.tree.roots) { node in
                    GroupBranch(node: node, ctx: context)
                }
            } header: {
                Text("Groups")
                    .padding(.vertical, 2)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(indicator?.groupID == nil && indicator != nil ? Color.accentColor.opacity(0.2) : .clear)
                    .onDrop(of: [.kudzuDrag], delegate: RootDropDelegate(library: library, browser: browser, indicator: $indicator))
            }
        }
        .listStyle(.sidebar)
        .safeAreaInset(edge: .bottom) {
            HStack {
                Button {
                    createGroup(under: nil)
                } label: {
                    Label("New Group", systemImage: "folder.badge.plus")
                }
                .buttonStyle(.borderless)
                .help("Create a group")
                Spacer()
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
        }
        .confirmationDialog(
            deleting.map { "Delete “\($0.name)”?" } ?? "",
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
            titleVisibility: .visible,
            presenting: deleting
        ) { group in
            deletionButtons(for: group)
        } message: { group in
            Text(deletionMessage(for: group))
        }
    }

    // MARK: - Selection

    private var scopeSelection: Binding<SidebarScope?> {
        Binding(
            get: { browser.scope },
            set: { if let scope = $0 { browser.scope = scope } }
        )
    }

    private func fixedRow(_ title: String, systemImage: String, scope: SidebarScope, count: Int) -> some View {
        Label(title, systemImage: systemImage)
            .badge(count)
            .tag(scope)
    }

    // MARK: - Context passed down the recursion

    private var context: GroupRowContext {
        GroupRowContext(
            library: library,
            browser: browser,
            collapsed: collapsedBinding,
            renaming: $renaming,
            renameText: $renameText,
            renameFocused: $renameFocused,
            indicator: $indicator,
            onNewGroup: createGroup(under:),
            onRename: beginRename,
            onDelete: requestDelete
        )
    }

    private var collapsedBinding: Binding<Set<UUID>> {
        Binding(
            get: { Set(collapsedRaw.split(separator: ",").compactMap { UUID(uuidString: String($0)) }) },
            set: { collapsedRaw = $0.map(\.uuidString).sorted().joined(separator: ",") }
        )
    }

    // MARK: - Actions

    private func createGroup(under parentID: UUID?) {
        guard let id = library.createGroup(under: parentID) else { return }
        if let parentID { collapsedBinding.wrappedValue.remove(parentID) }
        beginRename(id)
    }

    private func beginRename(_ id: UUID) {
        guard let group = library.tree.group(id) else { return }
        renameText = group.name
        renaming = id
        Task {
            try? await Task.sleep(for: .milliseconds(80))
            renameFocused = true
        }
    }

    private func requestDelete(_ group: GroupRecord) {
        let hasContents = (library.groupCounts[group.id] ?? 0) > 0 || !library.tree.children(of: group.id).isEmpty
        if hasContents {
            deleting = group
        } else {
            library.deleteGroup(group.id, .moveContentsUp)
            browser.prune(using: library)
        }
    }

    @ViewBuilder
    private func deletionButtons(for group: GroupRecord) -> some View {
        Button("Move Contents Up") {
            library.deleteGroup(group.id, .moveContentsUp)
            browser.prune(using: library)
        }
        Button("Delete Group and Contents", role: .destructive) {
            library.deleteGroup(group.id, .deleteContents)
            browser.prune(using: library)
        }
        Button("Cancel", role: .cancel) {}
    }

    private func deletionMessage(for group: GroupRecord) -> String {
        let snippets = library.groupCounts[group.id] ?? 0
        let subgroups = library.tree.descendantIDs(of: group.id, includingSelf: false).count
        var parts: [String] = []
        if snippets > 0 { parts.append("\(snippets) snippet\(snippets == 1 ? "" : "s")") }
        if subgroups > 0 { parts.append("\(subgroups) subgroup\(subgroups == 1 ? "" : "s")") }
        return "It contains \(parts.joined(separator: " and ")). Move them up a level, or delete them too? Deleting can't be undone."
    }
}

/// Everything a group row needs, bundled so the recursive row view stays small.
struct GroupRowContext {
    var library: LibraryModel
    var browser: BrowserState
    var collapsed: Binding<Set<UUID>>
    var renaming: Binding<UUID?>
    var renameText: Binding<String>
    var renameFocused: FocusState<Bool>.Binding
    var indicator: Binding<DropIndicator?>
    var onNewGroup: (UUID?) -> Void
    var onRename: (UUID) -> Void
    var onDelete: (GroupRecord) -> Void
}

/// A group row and, recursively, its subgroups.
private struct GroupBranch: View {
    let node: GroupNode
    let ctx: GroupRowContext

    var body: some View {
        if node.children.isEmpty {
            GroupRow(group: node.group, ctx: ctx)
                .tag(SidebarScope.group(node.id))
        } else {
            DisclosureGroup(isExpanded: expanded) {
                ForEach(node.children) { child in
                    GroupBranch(node: child, ctx: ctx)
                }
            } label: {
                GroupRow(group: node.group, ctx: ctx)
            }
            .tag(SidebarScope.group(node.id))
        }
    }

    private var expanded: Binding<Bool> {
        Binding(
            get: { !ctx.collapsed.wrappedValue.contains(node.id) },
            set: { isOpen in
                if isOpen { ctx.collapsed.wrappedValue.remove(node.id) } else { ctx.collapsed.wrappedValue.insert(node.id) }
            }
        )
    }
}

private struct GroupRow: View {
    let group: GroupRecord
    let ctx: GroupRowContext
    @State private var height: CGFloat = 28

    var body: some View {
        let isEnabled = ctx.library.tree.isEffectivelyEnabled(group.id)
        Group {
            if ctx.renaming.wrappedValue == group.id {
                HStack {
                    Image(systemName: "folder")
                    TextField("Name", text: ctx.renameText)
                        .textFieldStyle(.plain)
                        .focused(ctx.renameFocused)
                        .onSubmit(commitRename)
                        .onExitCommand { ctx.renaming.wrappedValue = nil }
                        .onChange(of: ctx.renameFocused.wrappedValue) { _, focused in
                            if !focused { commitRename() }
                        }
                }
            } else {
                Label(group.name, systemImage: isEnabled ? "folder" : "folder.badge.minus")
                    .badge(ctx.library.groupCounts[group.id] ?? 0)
            }
        }
        .opacity(isEnabled ? 1 : 0.5)
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { height = $0 }
        .overlay(alignment: .top) { line(for: .before) }
        .overlay(alignment: .bottom) { line(for: .after) }
        .background {
            if ctx.indicator.wrappedValue == DropIndicator(groupID: group.id, zone: .into) {
                RoundedRectangle(cornerRadius: 6).fill(Color.accentColor.opacity(0.25))
                    .padding(.horizontal, -6)
            }
        }
        .contextMenu { menu }
        .onDrag {
            ctx.browser.dragging = .group(group.id)
            return .kudzuDragMarker()
        }
        .onDrop(of: [.kudzuDrag], delegate: GroupDropDelegate(
            group: group, height: height, library: ctx.library, browser: ctx.browser, indicator: ctx.indicator
        ))
    }

    @ViewBuilder
    private func line(for zone: DropZone) -> some View {
        if ctx.indicator.wrappedValue == DropIndicator(groupID: group.id, zone: zone) {
            Capsule().fill(Color.accentColor).frame(height: 2).padding(.horizontal, -4)
        }
    }

    private func commitRename() {
        guard ctx.renaming.wrappedValue == group.id else { return }
        ctx.renaming.wrappedValue = nil
        ctx.library.renameGroup(group.id, to: ctx.renameText.wrappedValue)
    }

    @ViewBuilder
    private var menu: some View {
        Button("New Snippet Here") {
            ctx.browser.newSnippet(library: ctx.library, in: group.id)
        }
        Divider()
        Button("New Group") { ctx.onNewGroup(group.parentID) }
        Button("New Subgroup") { ctx.onNewGroup(group.id) }
        Divider()
        Button("Rename") { ctx.onRename(group.id) }
        Button(group.isEnabled ? "Disable" : "Enable") {
            ctx.library.setGroupEnabled(group.id, !group.isEnabled)
        }
        Divider()
        Button("Delete…", role: .destructive) { ctx.onDelete(group) }
    }
}

// MARK: - Drop handling

private struct GroupDropDelegate: DropDelegate {
    let group: GroupRecord
    let height: CGFloat
    let library: LibraryModel
    let browser: BrowserState
    @Binding var indicator: DropIndicator?

    private func zone(_ info: DropInfo) -> DropZone {
        guard case .group = browser.dragging else { return .into }
        let y = info.location.y
        if y < height * 0.28 { return .before }
        if y > height * 0.72 { return .after }
        return .into
    }

    /// Where a dragged group would end up for the given zone, or nil if that would be a cycle.
    private func plan(for dragged: UUID, zone: DropZone) -> (parent: UUID?, index: Int?)? {
        guard dragged != group.id else { return nil }
        let tree = library.tree
        let target: (parent: UUID?, index: Int?)
        switch zone {
        case .into:
            target = (group.id, nil)
        case .before, .after:
            let parent = tree.parent(of: group.id)
            let siblings = tree.children(of: parent).filter { $0.id != dragged }
            let position = siblings.firstIndex { $0.id == group.id } ?? siblings.count
            target = (parent, zone == .before ? position : position + 1)
        }
        return tree.canMove(dragged, toParent: target.parent) ? target : nil
    }

    private func isValid(_ info: DropInfo) -> Bool {
        guard info.hasItemsConforming(to: [.kudzuDrag]), let dragging = browser.dragging else { return false }
        switch dragging {
        case .snippets: return true
        case .group(let id): return plan(for: id, zone: zone(info)) != nil
        }
    }

    func validateDrop(info: DropInfo) -> Bool { isValid(info) }

    func dropEntered(info: DropInfo) {
        indicator = isValid(info) ? DropIndicator(groupID: group.id, zone: zone(info)) : nil
    }

    func dropUpdated(info: DropInfo) -> DropProposal? {
        let next = isValid(info) ? DropIndicator(groupID: group.id, zone: zone(info)) : nil
        if next != indicator { indicator = next }
        return DropProposal(operation: next == nil ? .forbidden : .move)
    }

    func dropExited(info: DropInfo) {
        if indicator?.groupID == group.id { indicator = nil }
    }

    func performDrop(info: DropInfo) -> Bool {
        defer {
            indicator = nil
            browser.dragging = nil
        }
        guard isValid(info), let dragging = browser.dragging else { return false }
        switch dragging {
        case .snippets(let ids):
            library.moveSnippets(ids, to: group.id)
            return true
        case .group(let id):
            guard let target = plan(for: id, zone: zone(info)) else { return false }
            return library.moveGroup(id, toParent: target.parent, at: target.index)
        }
    }
}

/// The "Groups" header: dropping a group makes it top-level, dropping snippets removes them from their group.
private struct RootDropDelegate: DropDelegate {
    let library: LibraryModel
    let browser: BrowserState
    @Binding var indicator: DropIndicator?

    private func isValid(_ info: DropInfo) -> Bool {
        guard info.hasItemsConforming(to: [.kudzuDrag]), let dragging = browser.dragging else { return false }
        switch dragging {
        case .snippets: return true
        case .group(let id): return library.tree.canMove(id, toParent: nil)
        }
    }

    func validateDrop(info: DropInfo) -> Bool { isValid(info) }

    func dropEntered(info: DropInfo) {
        indicator = isValid(info) ? DropIndicator(groupID: nil, zone: .into) : nil
    }

    func dropUpdated(info: DropInfo) -> DropProposal? {
        DropProposal(operation: isValid(info) ? .move : .forbidden)
    }

    func dropExited(info: DropInfo) {
        if indicator?.groupID == nil { indicator = nil }
    }

    func performDrop(info: DropInfo) -> Bool {
        defer {
            indicator = nil
            browser.dragging = nil
        }
        guard isValid(info), let dragging = browser.dragging else { return false }
        switch dragging {
        case .snippets(let ids):
            library.moveSnippets(ids, to: nil)
            return true
        case .group(let id):
            return library.moveGroup(id, toParent: nil, at: nil)
        }
    }
}
