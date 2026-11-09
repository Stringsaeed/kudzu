import Foundation

public struct GroupNode: Hashable, Sendable {
    public var group: Group
    public var depth: Int
    public var children: [GroupNode]
}
/// Nesting, ordering and effective enablement, matching groups.ts (including cycle handling).
public enum Groups {
    public static func buildTree(_ groups: [Group]) -> [GroupNode] {
        let ids = Set(groups.map(\.id)); var byParent: [UUID?: [Group]] = [:], seen: Set<UUID> = []
        for g in groups { let parent = g.parentID.flatMap { ids.contains($0) && $0 != g.id ? $0 : nil }; byParent[parent, default: []].append(g) }
        func walk(_ parent: UUID?, _ depth: Int) -> [GroupNode] {
            let sorted = (byParent[parent] ?? []).sorted { $0.sortIndex != $1.sortIndex ? $0.sortIndex < $1.sortIndex : $0.name.localizedCompare($1.name) == .orderedAscending }
            // JS filters the full sibling list before recursing.
            let unique = sorted.filter { seen.insert($0.id).inserted }
            return unique.map { GroupNode(group: $0, depth: depth, children: walk($0.id, depth + 1)) }
        }
        return walk(nil, 0)
    }
    public static func flatten(_ nodes: [GroupNode], collapsed: Set<UUID> = []) -> [GroupNode] {
        nodes.flatMap { [$0] + (collapsed.contains($0.group.id) ? [] : flatten($0.children, collapsed: collapsed)) }
    }
    public static func descendantIds(_ groups: [Group], id: UUID) -> Set<UUID> {
        var out: Set<UUID> = [id], grew = true
        while grew { grew = false; for g in groups { if let p = g.parentID, out.contains(p), out.insert(g.id).inserted { grew = true } } }
        return out
    }
    public static func groupPath(_ groups: [Group], id: UUID?) -> [String] {
        let byId = Dictionary(groups.map { ($0.id, $0) }, uniquingKeysWith: { _, b in b })
        var path: [String] = [], seen: Set<UUID> = [], cur = id.flatMap { byId[$0] }
        while let g = cur, seen.insert(g.id).inserted { path.insert(g.name, at: 0); cur = g.parentID.flatMap { byId[$0] } }
        return path
    }
    public static func isGroupEnabled(_ groups: [Group], id: UUID?) -> Bool {
        let byId = Dictionary(groups.map { ($0.id, $0) }, uniquingKeysWith: { _, b in b })
        var seen: Set<UUID> = [], cur = id.flatMap { byId[$0] }
        while let g = cur, seen.insert(g.id).inserted { if !g.isEnabled { return false }; cur = g.parentID.flatMap { byId[$0] } }
        return true
    }
    public static func recursiveCounts(_ groups: [Group], snippets: [Snippet]) -> [UUID: Int] {
        let byId = Dictionary(groups.map { ($0.id, $0) }, uniquingKeysWith: { _, b in b }); var counts: [UUID: Int] = [:]
        for s in snippets {
            var seen: Set<UUID> = [], id = s.groupID
            while let key = id, let group = byId[key], seen.insert(key).inserted { counts[key, default: 0] += 1; id = group.parentID }
        }
        return counts
    }
}
