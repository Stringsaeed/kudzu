import KudzuCore
import KudzuUI
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

/// The sidebar's sections: the library views, then the nested group tree.
///
/// Rows are `KzSidebarItem`-style buttons on the glass, as the design system draws them, rather than
/// an outline `List`: the shell's sidebar is a scrolling stack. Groups keep their native behaviours:
/// drag to reorder or nest (and drop snippets on them), right-click for the group menu, ← and → to
/// collapse and expand, and an inline rename field.
struct SidebarView: View {
    @Environment(LibraryModel.self) private var library
    @Environment(BrowserState.self) private var browser

    /// In settings mode no library item is active.
    var isSettings: Bool
    /// Chooses a scope and leaves settings.
    var choose: (SidebarScope) -> Void

    @SceneStorage("sidebar.collapsedGroups") private var collapsedRaw = ""
    @State private var renaming: UUID?
    @State private var renameText = ""
    @State private var deleting: GroupRecord?
    @State private var editingRules: GroupRecord?
    @State private var indicator: DropIndicator?
    @State private var contextTarget: UUID?
    @FocusState private var renameFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: KzSpace.s4) {
            KzSidebarSection {
                libraryItem("All snippets", icon: .stack, scope: .all, count: library.snippets.count)
                libraryItem("Favorites", icon: .star, scope: .favorites,
                            count: library.snippets.lazy.filter(\.isFavorite).count)
                libraryItem("Recently used", icon: .clock, scope: .recent, count: nil)
                libraryItem("Stats", icon: .chart, scope: .stats, count: nil)
            }

            groupsSection
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
        .sheet(item: $editingRules) { group in
            GroupRulesSheet(groupID: group.id)
                .environment(library)
        }
    }

    private func libraryItem(_ title: String, icon: KzIconName, scope: SidebarScope, count: Int?) -> some View {
        KzSidebarItem(title, icon: icon, count: count, isActive: !isSettings && browser.scope == scope) {
            choose(scope)
        }
    }

    // MARK: - Groups

    private var groupsSection: some View {
        let rootTargeted = indicator == DropIndicator(groupID: nil, zone: .into)
        return VStack(alignment: .leading, spacing: 0) {
            KzSidebarSection("Groups", action: {
                KzIconButton(.plus, label: "New group", size: .sm) { createGroup(under: nil) }
            }) {
                ForEach(library.tree.roots) { node in
                    GroupBranch(node: node, depth: 0, ctx: context)
                }
            }
            // Dropping below the groups (or on the heading) moves a group to the top level and takes snippets out of their group.
            Color.clear
                .frame(maxWidth: .infinity, minHeight: 28)
                .contentShape(Rectangle())
                .liveOnly { $0.onDrop(of: [.kudzuDrag], delegate: RootDropDelegate(library: library, browser: browser, indicator: $indicator)) }
        }
        .overlay(alignment: .top) {
            // The heading row, leaving its + button clear.
            Color.clear
                .frame(height: 22)
                .contentShape(Rectangle())
                .padding(.trailing, 36)
                .liveOnly { $0.onDrop(of: [.kudzuDrag], delegate: RootDropDelegate(library: library, browser: browser, indicator: $indicator)) }
        }
        .background(alignment: .top) {
            if rootTargeted {
                RoundedRectangle(cornerRadius: KzRadius.md).fill(KzColor.vineSoft).frame(height: 22)
            }
        }
    }

    private var context: GroupRowContext {
        GroupRowContext(
            library: library,
            browser: browser,
            isSettings: isSettings,
            showsDisclosure: library.tree.allGroups.contains { !library.tree.children(of: $0.id).isEmpty },
            collapsed: collapsedBinding,
            renaming: $renaming,
            renameText: $renameText,
            renameFocused: $renameFocused,
            indicator: $indicator,
            contextTarget: $contextTarget,
            choose: choose,
            onNewGroup: createGroup(under:),
            onRename: beginRename,
            onEditRules: { editingRules = $0 },
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
        Button("Move contents up") {
            library.deleteGroup(group.id, .moveContentsUp)
            browser.prune(using: library)
        }
        Button("Delete group and contents", role: .destructive) {
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
    var isSettings: Bool
    /// Whether any group has subgroups; rows then keep a chevron column so names line up.
    var showsDisclosure: Bool
    var collapsed: Binding<Set<UUID>>
    var renaming: Binding<UUID?>
    var renameText: Binding<String>
    var renameFocused: FocusState<Bool>.Binding
    var indicator: Binding<DropIndicator?>
    var contextTarget: Binding<UUID?>
    var choose: (SidebarScope) -> Void
    var onNewGroup: (UUID?) -> Void
    var onRename: (UUID) -> Void
    var onEditRules: (GroupRecord) -> Void
    var onDelete: (GroupRecord) -> Void
}

/// A group row and, recursively, its subgroups.
private struct GroupBranch: View {
    let node: GroupNode
    let depth: Int
    let ctx: GroupRowContext

    var body: some View {
        let isExpanded = !ctx.collapsed.wrappedValue.contains(node.id)
        GroupRow(group: node.group, depth: depth, hasChildren: !node.children.isEmpty, isExpanded: isExpanded,
                 setExpanded: setExpanded, ctx: ctx)
        if isExpanded {
            ForEach(node.children) { child in
                GroupBranch(node: child, depth: depth + 1, ctx: ctx)
            }
        }
    }

    private func setExpanded(_ isOpen: Bool) {
        if isOpen { ctx.collapsed.wrappedValue.remove(node.id) } else { ctx.collapsed.wrappedValue.insert(node.id) }
    }
}

private struct GroupRow: View {
    let group: GroupRecord
    let depth: Int
    let hasChildren: Bool
    let isExpanded: Bool
    let setExpanded: (Bool) -> Void
    let ctx: GroupRowContext
    @State private var height: CGFloat = KzSize.control

    private static let indent: CGFloat = 14
    private static let chevronColumn: CGFloat = 16

    private var isActive: Bool { !ctx.isSettings && ctx.browser.scope == .group(group.id) }
    private var leadingInset: CGFloat {
        CGFloat(depth) * Self.indent + (ctx.showsDisclosure ? Self.chevronColumn : 0)
    }

    var body: some View {
        let isEnabled = ctx.library.tree.isEffectivelyEnabled(group.id)
        Group {
            if ctx.renaming.wrappedValue == group.id {
                renameField
            } else {
                row(isEnabled: isEnabled)
            }
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { height = $0 }
        .overlay(alignment: .top) { line(for: .before) }
        .overlay(alignment: .bottom) { line(for: .after) }
        .overlay {
            if ctx.indicator.wrappedValue == DropIndicator(groupID: group.id, zone: .into) {
                RoundedRectangle(cornerRadius: KzRadius.md).strokeBorder(KzColor.vineInk, lineWidth: 2)
                    .background(KzColor.vineSoft, in: RoundedRectangle(cornerRadius: KzRadius.md))
                    .allowsHitTesting(false)
            }
        }
        .liveOnly {
            $0.onDrag {
                ctx.browser.dragging = .group(group.id)
                return .kudzuDragMarker()
            }
            .onDrop(of: [.kudzuDrag], delegate: GroupDropDelegate(
                group: group, height: height, library: ctx.library, browser: ctx.browser, indicator: ctx.indicator
            ))
        }
    }

    private func row(isEnabled: Bool) -> some View {
        Button { ctx.choose(.group(group.id)) } label: {
            HStack(spacing: 10) {
                KzIcon(isEnabled ? .folder : .folderMinus, size: 16)
                    .foregroundStyle(isActive ? KzColor.vineInk : KzColor.inkMuted)
                Text(group.name)
                    .kzTextStyle(.bodyStrong, matchLineBox: false)
                    .foregroundStyle(isEnabled ? KzColor.ink : KzColor.inkMuted)
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                let count = ctx.library.groupCounts[group.id] ?? 0
                if count > 0 {
                    Text("\(count)")
                        .kzTextStyle(.caption, matchLineBox: false)
                        .monospacedDigit()
                        .foregroundStyle(KzColor.inkMuted)
                }
            }
            .padding(.leading, leadingInset)
        }
        .buttonStyle(KzSidebarItemStyle(isActive: isActive))
        .overlay(alignment: .leading) {
            if hasChildren {
                Button { setExpanded(!isExpanded) } label: {
                    KzIcon(.chevronRight, size: 12)
                        .rotationEffect(.degrees(isExpanded ? 90 : 0))
                        .kzAnimation(.color, value: isExpanded)
                        .frame(width: Self.chevronColumn, height: KzSize.control)
                        .contentShape(Rectangle())
                }
                .buttonStyle(ChevronStyle())
                .padding(.leading, 10 + CGFloat(depth) * Self.indent - 2)
                .accessibilityLabel(isExpanded ? "Collapse \(group.name)" : "Expand \(group.name)")
            }
        }
        .onKeyPress(.leftArrow) {
            guard hasChildren, isExpanded else { return .ignored }
            setExpanded(false)
            return .handled
        }
        .onKeyPress(.rightArrow) {
            guard hasChildren, !isExpanded else { return .ignored }
            setExpanded(true)
            return .handled
        }
        .accessibilityAddTraits(isActive ? .isSelected : [])
        .accessibilityValue(isEnabled ? "" : "Disabled")
        .kzContextMenu(isPresented: contextBinding) { menu }
        .kzContextTarget(ctx.contextTarget.wrappedValue == group.id)
    }

    private var renameField: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.md)
        return HStack(spacing: 10) {
            KzIcon(.folder, size: 16).foregroundStyle(KzColor.inkMuted)
            TextField("Group name", text: ctx.renameText)
                .textFieldStyle(.plain)
                .font(KzTextStyle.bodyStrong.font)
                .foregroundStyle(KzColor.ink)
                .focused(ctx.renameFocused)
                .onSubmit(commitRename)
                .onExitCommand { ctx.renaming.wrappedValue = nil }
                .onChange(of: ctx.renameFocused.wrappedValue) { _, focused in
                    if !focused { commitRename() }
                }
        }
        .padding(.leading, 10 + leadingInset)
        .padding(.trailing, 10)
        .frame(height: KzSize.control)
        .background(KzColor.surfaceRaised, in: shape)
        .overlay(shape.strokeBorder(KzColor.vineInk, lineWidth: 2))
    }

    private var contextBinding: Binding<Bool> {
        Binding(
            get: { ctx.contextTarget.wrappedValue == group.id },
            set: { open in
                if open {
                    ctx.contextTarget.wrappedValue = group.id
                } else if ctx.contextTarget.wrappedValue == group.id {
                    ctx.contextTarget.wrappedValue = nil
                }
            }
        )
    }

    @ViewBuilder
    private func line(for zone: DropZone) -> some View {
        if ctx.indicator.wrappedValue == DropIndicator(groupID: group.id, zone: zone) {
            Capsule().fill(KzColor.vineInk).frame(height: 2).padding(.leading, leadingInset)
        }
    }

    private func commitRename() {
        guard ctx.renaming.wrappedValue == group.id else { return }
        ctx.renaming.wrappedValue = nil
        ctx.library.renameGroup(group.id, to: ctx.renameText.wrappedValue)
    }

    @KzMenuBuilder
    private var menu: [KzMenuElement] {
        KzMenuItem("New snippet here", icon: .compose) {
            ctx.choose(.group(group.id))
            ctx.browser.newSnippet(library: ctx.library, in: group.id)
        }
        KzMenuSeparator()
        KzMenuItem("New group", icon: .folderPlus) { ctx.onNewGroup(group.parentID) }
        KzMenuItem("New subgroup", icon: .folderPlus) { ctx.onNewGroup(group.id) }
        KzMenuSeparator()
        KzMenuItem("Rename", icon: .type) { ctx.onRename(group.id) }
        KzMenuItem("Rules…", icon: .app) { ctx.onEditRules(group) }
        KzMenuItem(group.isEnabled ? "Disable" : "Enable", icon: group.isEnabled ? .pause : .play) {
            ctx.library.setGroupEnabled(group.id, !group.isEnabled)
        }
        KzMenuSeparator()
        KzMenuItem("Delete…", icon: .trash, role: .danger) { ctx.onDelete(group) }
    }

    /// The disclosure chevron: `ink-muted`, `ink` on hover, no fill of its own.
    private struct ChevronStyle: ButtonStyle {
        func makeBody(configuration: Configuration) -> some View {
            KzInteractive(RoundedRectangle(cornerRadius: KzRadius.xs), isPressed: configuration.isPressed, stateLayer: false) { state in
                configuration.label
                    .foregroundStyle(state.isHovered ? KzColor.ink : KzColor.inkMuted)
            }
        }
    }
}

/// Edits a group's `AppRule` and `InsertionMethod` override with `RulesSection`, in a sheet on `surface-raised`.
private struct GroupRulesSheet: View {
    @Environment(LibraryModel.self) private var library
    @Environment(\.dismiss) private var dismiss
    let groupID: UUID

    private var group: GroupRecord? {
        library.tree.group(groupID)
    }

    private var ruleBinding: Binding<AppRule?> {
        Binding(
            get: { group?.rule },
            set: { newRule in
                library.setGroupRules(groupID, rule: newRule, insertion: group?.insertion)
            }
        )
    }

    private var insertionBinding: Binding<InsertionMethod?> {
        Binding(
            get: { group?.insertion },
            set: { newInsertion in
                library.setGroupRules(groupID, rule: group?.rule, insertion: newInsertion)
            }
        )
    }

    var body: some View {
        VStack(alignment: .leading, spacing: KzSpace.s5) {
            HStack(spacing: KzSpace.s2) {
                KzIcon(.folder, size: 16).foregroundStyle(KzColor.inkMuted)
                Text(group.map { "Rules for \($0.name)" } ?? "Group rules")
                    .kzTextStyle(.title)
                    .foregroundStyle(KzColor.ink)
                    .lineLimit(1)
                    .accessibilityAddTraits(.isHeader)
                Spacer(minLength: KzSpace.s2)
            }

            Text("Snippets and subgroups in this group inherit these settings unless they set their own.")
                .kzTextStyle(.callout)
                .foregroundStyle(KzColor.inkMuted)
                .fixedSize(horizontal: false, vertical: true)

            RulesSection(
                rule: ruleBinding,
                insertion: insertionBinding,
                inherited: library.tree.inheritedSettings(forGroupItself: groupID),
                defaultInsertion: AppMatchingSettings.shared.defaultInsertion,
                inheritOptionTitle: group?.parentID == nil ? "All apps" : "Inherit from group"
            )

            HStack {
                Spacer()
                KzButton("Done", variant: .primary, isDefault: true) { dismiss() }
            }
        }
        .padding(KzSpace.s6)
        .frame(width: 480, alignment: .leading)
        .background(KzColor.surfaceRaised)
        .presentationBackground(KzColor.surfaceRaised)
    }
}

// MARK: - Drop handling

extension View {
    /// Applies `transform` except in offscreen snapshots, where drag and drop draw AppKit placeholders.
    func liveOnly(@ViewBuilder _ transform: (Self) -> some View) -> some View {
        modifier(LiveOnlyModifier(live: transform(self), snapshot: self))
    }
}

private struct LiveOnlyModifier<Live: View, Snapshot: View>: ViewModifier {
    let live: Live
    let snapshot: Snapshot
    @Environment(\.kzSnapshotRendering) private var snapshotRendering

    func body(content: Content) -> some View {
        if snapshotRendering { snapshot } else { live }
    }
}

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

/// The "Groups" heading and the space below the groups: dropping a group makes it top-level,
/// dropping snippets removes them from their group.
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
