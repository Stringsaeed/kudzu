// The groups tree in the sidebar (`GroupRow`, `RenameRow`): nested `DisclosureGroup`s (native
// chevrons and ←/→ folding), counts that include subgroups, drop targets for snippets and groups,
// inline rename, and the group context menu.

import KudzuCore
import KudzuDesign
import SwiftUI

/// The rows for `nodes` and, recursively, their children.
struct GroupTreeRows: View {
    var nodes: [GroupNode]
    var counts: [UUID: Int]
    @Binding var collapsed: Set<UUID>

    var body: some View {
        ForEach(nodes, id: \.group.id) { node in
            if node.children.isEmpty {
                GroupRowContent(node: node, count: counts[node.group.id], expanded: false)
                    .tag(Route.group(id: node.group.id))
                    .listRowInsets(EdgeInsets(top: 2, leading: 0, bottom: 2, trailing: 0))
            } else {
                DisclosureGroup(isExpanded: expansion(node.group.id)) {
                    GroupTreeRows(nodes: node.children, counts: counts, collapsed: $collapsed)
                } label: {
                    GroupRowContent(node: node, count: counts[node.group.id], expanded: !collapsed.contains(node.group.id))
                }
                .tag(Route.group(id: node.group.id))
                .listRowInsets(EdgeInsets(top: 2, leading: 0, bottom: 2, trailing: 0))
            }
        }
    }

    private func expansion(_ id: UUID) -> Binding<Bool> {
        Binding(get: { !collapsed.contains(id) }, set: { open in
            if open { collapsed.remove(id) } else { collapsed.insert(id) }
        })
    }
}

/// One group's row: folder icon, name (or the rename field), the "off" mark and the count.
struct GroupRowContent: View {
    var node: GroupNode
    var count: Int?
    var expanded: Bool
    @Environment(AppModel.self) private var model
    @Environment(LibraryActions.self) private var actions
    @State private var over = false

    var body: some View {
        let g = node.group
        Group {
            if actions.renaming == g.id {
                RenameField(group: g)
            } else {
                SidebarRow(icon: !node.children.isEmpty && expanded ? Icon.folderOpen : Icon.folder, label: g.name, count: count, selected: LibraryRoutes.sidebarTag(model.view) == .group(id: g.id), muted: !g.isEnabled, dropTarget: over) {
                    if !g.isEnabled {
                        Icon.circleOff.icon(12).foregroundStyle(.token(.faint)).accessibilityLabel("Off")
                    }
                }
                .onTapGesture(count: 2) {
                    if GroupRenameGesture.startsRename(clickCount: 2) { actions.renaming = g.id }
                }
                .draggable(LibraryDrag.group(g)) {
                    Label(g.name, systemImage: "folder").font(.system(size: 12)).padding(.horizontal, 8).padding(.vertical, 4)
                }
                .dropDestination(for: LibraryDrag.self) { items, _ in
                    actions.drop(items, on: g.id)
                } isTargeted: { over = $0 }
                .contextMenu { GroupMenu(group: g) }
            }
        }
    }
}

/// The group context menu (`ContextMenuContent` in `GroupRow`).
struct GroupMenu: View {
    var group: SnippetGroup
    @Environment(LibraryActions.self) private var actions

    var body: some View {
        Button { actions.renaming = group.id } label: { Label("Rename", systemImage: "pencil") }
        Button { actions.newGroup(parent: group.id) } label: { Label("New subgroup", systemImage: "folder.badge.plus") }
        Button { actions.openGroupSettings(group) } label: { Label("Group settings…", systemImage: "slider.horizontal.3") }
        Button {
            actions.perform { try await actions.model.toggleGroupEnabled(group.id) }
        } label: {
            group.isEnabled ? Label("Turn off group", systemImage: "circle.slash") : Label("Turn on group", systemImage: "play")
        }
        if group.parentID != nil {
            Button {
                actions.perform { _ = try await actions.model.moveGroup(group.id, into: nil) }
            } label: { Label("Move to top level", systemImage: "square.3.layers.3d") }
        }
        Divider()
        Button(role: .destructive) {
            actions.perform { try await actions.model.deleteGroup(group.id, deleteSnippets: false) }
        } label: { Label("Delete group, keep snippets", systemImage: "trash") }
        Button(role: .destructive) {
            actions.perform { try await actions.model.deleteGroup(group.id, deleteSnippets: true) }
        } label: { Label("Delete group and snippets", systemImage: "trash") }
    }
}

/// Inline rename (`RenameRow`): selected on appear; Return or leaving the field saves, Esc cancels.
struct RenameField: View {
    var group: SnippetGroup
    @Environment(LibraryActions.self) private var actions
    @State private var name: String
    @State private var done = false
    @FocusState private var focused: Bool

    init(group: SnippetGroup) {
        self.group = group
        _name = State(initialValue: group.name)
    }

    var body: some View {
        HStack(spacing: 8) {
            Icon.folder.icon(16).foregroundStyle(.token(.iris))
            TextField("Group name", text: $name)
                .textFieldStyle(.plain)
                .font(.system(size: 13))
                .focused($focused)
                .onSubmit { finish(name) }
                .onExitCommand { finish(nil) }
                .accessibilityLabel("Group name")
        }
        .padding(.horizontal, 2)
        .frame(height: 24)
        .onAppear { focused = true }
        .onChange(of: focused) { _, now in if !now { finish(name) } }
    }

    private func finish(_ value: String?) {
        guard !done else { return }
        done = true
        actions.commitRename(group.id, name: value?.trimmingCharacters(in: .whitespacesAndNewlines))
    }
}
