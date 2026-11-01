import Foundation

/// One group and its sorted subgroups, for outline views.
public struct GroupNode: Identifiable, Hashable, Sendable {
    public var group: GroupRecord
    public var children: [GroupNode]

    public var id: UUID { group.id }
    /// `nil` for leaves, as `OutlineGroup`/`List(children:)` expect.
    public var optionalChildren: [GroupNode]? { children.isEmpty ? nil : children }
}

/// An immutable, cycle-safe view of the group hierarchy with move planning.
///
/// Synced data can arrive with dangling parents or even cycles (two devices nesting groups into
/// each other). Dangling parents are treated as top-level, and a cycle is broken by promoting its
/// first member to top level, so every group is always reachable exactly once.
public struct GroupTree: Sendable {
    /// Top-level groups in display order, with their subtrees.
    public private(set) var roots: [GroupNode] = []

    private var byID: [UUID: GroupRecord] = [:]
    private var childIDs: [UUID?: [UUID]] = [:]
    private var effectiveParent: [UUID: UUID] = [:]

    /// Builds the tree from flat group records.
    public init(groups: [GroupRecord]) {
        for group in groups where byID[group.id] == nil { byID[group.id] = group }
        let ordered = byID.values.sorted(by: Self.displayOrder)

        var rawChildren: [UUID?: [UUID]] = [:]
        for group in ordered {
            let parent = group.parentID.flatMap { byID[$0] != nil && $0 != group.id ? $0 : nil }
            rawChildren[parent, default: []].append(group.id)
        }

        var visited = Set<UUID>()
        func build(_ id: UUID, parent: UUID?) -> GroupNode? {
            guard visited.insert(id).inserted, let group = byID[id] else { return nil }
            if let parent { effectiveParent[id] = parent }
            childIDs[parent, default: []].append(id)
            let kids = (rawChildren[id] ?? []).compactMap { build($0, parent: id) }
            return GroupNode(group: group, children: kids)
        }
        roots = (rawChildren[nil] ?? []).compactMap { build($0, parent: nil) }
        // Anything left over sits in a cycle: promote members one at a time until all are placed.
        for group in ordered where !visited.contains(group.id) {
            if let node = build(group.id, parent: nil) { roots.append(node) }
        }
    }

    private static func displayOrder(_ a: GroupRecord, _ b: GroupRecord) -> Bool {
        if a.sortIndex != b.sortIndex { return a.sortIndex < b.sortIndex }
        let na = SnippetSearchIndex.fold(a.name), nb = SnippetSearchIndex.fold(b.name)
        if na != nb { return na < nb }
        return a.id.uuidString < b.id.uuidString
    }

    // MARK: - Lookup

    /// The group with this ID.
    public func group(_ id: UUID) -> GroupRecord? { byID[id] }

    /// Every group, depth-first in display order.
    public var allGroups: [GroupRecord] {
        var result: [GroupRecord] = []
        func walk(_ nodes: [GroupNode]) { for node in nodes { result.append(node.group); walk(node.children) } }
        walk(roots)
        return result
    }

    /// Direct subgroups of `parentID` (nil = top level) in display order.
    public func children(of parentID: UUID?) -> [GroupRecord] {
        (childIDs[parentID] ?? []).compactMap { byID[$0] }
    }

    /// The group's parent after cycle repair.
    public func parent(of id: UUID) -> UUID? { effectiveParent[id] }

    /// Ancestors of a group, nearest first.
    public func ancestorIDs(of id: UUID) -> [UUID] {
        var result: [UUID] = []
        var current = effectiveParent[id]
        while let parent = current {
            result.append(parent)
            current = effectiveParent[parent]
        }
        return result
    }

    /// IDs of every group below `id`, optionally including `id`.
    public func descendantIDs(of id: UUID, includingSelf: Bool = true) -> Set<UUID> {
        var result: Set<UUID> = includingSelf && byID[id] != nil ? [id] : []
        var stack = childIDs[id] ?? []
        while let next = stack.popLast() {
            if result.insert(next).inserted { stack += childIDs[next] ?? [] }
        }
        return result
    }

    /// Slash-free breadcrumb of names from the top level down to `id`.
    public func path(to id: UUID) -> [String] {
        (ancestorIDs(of: id).reversed() + [id]).compactMap { byID[$0]?.name }
    }

    /// Whether the group and all its ancestors are enabled.
    public func isEffectivelyEnabled(_ id: UUID) -> Bool {
        guard let group = byID[id], group.isEnabled else { return false }
        return ancestorIDs(of: id).allSatisfy { byID[$0]?.isEnabled ?? true }
    }

    /// The nearest rule set on `groupID` or one of its ancestors, with the group that sets it.
    public func inheritedRule(forGroup groupID: UUID?) -> (rule: AppRule, from: GroupRecord)? {
        guard let groupID else { return nil }
        for id in [groupID] + ancestorIDs(of: groupID) {
            if let group = byID[id], let rule = group.rule { return (rule, group) }
        }
        return nil
    }

    /// The nearest insertion override on `groupID` or one of its ancestors, with the group that sets it.
    public func inheritedInsertion(forGroup groupID: UUID?) -> (method: InsertionMethod, from: GroupRecord)? {
        guard let groupID else { return nil }
        for id in [groupID] + ancestorIDs(of: groupID) {
            if let group = byID[id], let method = group.insertion { return (method, group) }
        }
        return nil
    }

    // MARK: - Counts

    /// Snippets per group including everything in its subgroups. Snippets in unknown groups count as ungrouped.
    public func recursiveSnippetCounts(_ snippets: [SnippetRecord]) -> [UUID: Int] {
        var counts: [UUID: Int] = [:]
        for snippet in snippets {
            guard let groupID = snippet.groupID, byID[groupID] != nil else { continue }
            counts[groupID, default: 0] += 1
            for ancestor in ancestorIDs(of: groupID) { counts[ancestor, default: 0] += 1 }
        }
        return counts
    }

    // MARK: - Moves

    /// Whether `id` may be placed under `newParent` (nil = top level) without creating a cycle.
    public func canMove(_ id: UUID, toParent newParent: UUID?) -> Bool {
        guard byID[id] != nil else { return false }
        guard let newParent else { return true }
        guard byID[newParent] != nil else { return false }
        return !descendantIDs(of: id).contains(newParent)
    }

    /// The `sortIndex` for a new group appended under `parentID`.
    public func nextSortIndex(under parentID: UUID?) -> Int {
        (children(of: parentID).map(\.sortIndex).max() ?? -1) + 1
    }

    /// Plans moving `id` under `newParent` at `index` among its new siblings, not counting `id` itself (nil = last).
    ///
    /// Returns only the records that change (the moved group plus siblings that are renumbered),
    /// or `nil` if the move is impossible or would create a cycle.
    public func reparent(_ id: UUID, toParent newParent: UUID?, at index: Int? = nil) -> [GroupRecord]? {
        guard canMove(id, toParent: newParent), var moved = byID[id] else { return nil }
        var siblings = children(of: newParent).filter { $0.id != id }
        let position = min(max(index ?? siblings.count, 0), siblings.count)
        moved.parentID = newParent
        siblings.insert(moved, at: position)

        var changed: [GroupRecord] = []
        for (i, var sibling) in siblings.enumerated() {
            let original = byID[sibling.id]
            sibling.sortIndex = i
            if sibling.id == id {
                if original != sibling { changed.append(sibling) }
            } else if original?.sortIndex != i {
                changed.append(sibling)
            }
        }
        return changed
    }
}
