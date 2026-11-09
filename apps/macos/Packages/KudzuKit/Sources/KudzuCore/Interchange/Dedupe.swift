// Ported from kudzu-core/src/interchange/dedupe.rs
// Finds duplicates created by syncing or repeated imports. Pure, deterministic and idempotent.
//
// Every device runs this independently after remote changes, so the survivor choice depends
// only on the data (never on input order or the device) and all devices converge.
// UUIDs tie-break by their natural order, which matches comparing the Swift app's uppercase
// UUID strings.

import Foundation

/// Deterministic, idempotent duplicate merging. Survivor choices depend only on data.
public enum Dedupe {
    public typealias Conflict = DedupeConflict
    public struct UsageMerge: Equatable, Sendable {
        public var snippetID: UUID; public var deviceID: String; public var sourceSnippetIDs: [UUID]
        public var count: Int; public var charsSaved: Int; public var lastUsedAt: Date?
        public var record: UsageRecord { UsageRecord(snippetID:snippetID,deviceID:deviceID,count:count,charsSaved:charsSaved,lastUsedAt:lastUsedAt) }
        func covers(_ row: UsageRecord) -> Bool { row.deviceID.isExactly(deviceID) && sourceSnippetIDs.contains(row.snippetID) }
    }
    public typealias Report = DedupeReport
    public struct Plan: Equatable, Sendable {
        public init() {}
        public var groupMerges: [UUID:UUID] = [:], snippetMerges: [UUID:UUID] = [:]
        public var updatedGroups: [Group] = [], updatedSnippets: [Snippet] = []
        public var usageMerges: [UsageMerge] = [], conflicts: [Conflict] = []
        public var isEmpty: Bool { groupMerges.isEmpty && snippetMerges.isEmpty && updatedGroups.isEmpty && updatedSnippets.isEmpty && usageMerges.isEmpty }
        public var report: Report { Report(mergedGroups:groupMerges.count,mergedSnippets:snippetMerges.count,mergedUsageRows:usageMerges.count,conflicts:conflicts) }
        /// The library after applying the plan.
        public func applied(_ library: Library) -> Library {
            guard !isEmpty else { return library }
            var out = library
            out.groups = library.groups.filter { groupMerges[$0.id] == nil }.map { g in updatedGroups.first { $0.id == g.id } ?? g }
            out.snippets = library.snippets.filter { snippetMerges[$0.id] == nil }.map { s in updatedSnippets.first { $0.id == s.id } ?? s }
            out.usage = library.usage.filter { row in !usageMerges.contains { $0.covers(row) } } + usageMerges.map(\.record)
            return out
        }
    }
    /// Triggers as a set: lowercased, de-duplicated and sorted.
    public static func normalizedTriggers(_ triggers: [String]) -> [String] { Set(triggers.map { Data($0.lowercased().utf8) }).sorted { $0.lexicographicallyPrecedes($1) }.map { String(decoding:$0,as:UTF8.self) } }
    struct GroupKey: Hashable { var parent: UUID?; var name: Data }
    struct SnippetKey: Hashable { var triggers: [String]; var content: Data; var formatting: Data? }
    struct UsageKey: Hashable { var id: UUID; var device: Data }
    /// Aliases repoint delayed usage rows whose snippets were merged by an earlier pass.
    public static func plan(_ library: Library, aliases: [UUID:UUID] = [:]) -> Plan {
        var p = Plan(), groups: [UUID:Group] = [:], snippets: [UUID:Snippet] = [:]
        for g in library.groups where groups[g.id] == nil { groups[g.id] = g }
        for s in library.snippets where snippets[s.id] == nil { snippets[s.id] = s }
        func resolve(_ id: UUID) -> UUID {
            var cur = id, seen: Set<UUID> = []
            while let next = p.groupMerges[cur], seen.insert(cur).inserted { cur = next }
            return cur
        }
        while true {
            let buckets = Dictionary(grouping:groups.values.filter { p.groupMerges[$0.id] == nil }) { GroupKey(parent:$0.parentID.map(resolve),name:Data(RustText.trim($0.name).lowercased().utf8)) }
            var changed = false
            for bucket in buckets.values where bucket.count > 1 {
                let ids = bucket.map(\.id).sorted { $0.lowercased < $1.lowercased }
                for id in ids.dropFirst() { p.groupMerges[id] = ids[0]; changed = true }
            }
            if !changed { break }
        }
        for id in Array(p.groupMerges.keys) { p.groupMerges[id] = resolve(id) }
        for var g in groups.values where p.groupMerges[g.id] == nil {
            let parent = g.parentID.map(resolve)
            if parent != g.parentID { g.parentID = parent; p.updatedGroups.append(g) }
        }
        p.updatedGroups.sort { $0.id.lowercased < $1.id.lowercased }
        let buckets = Dictionary(grouping:snippets.values) { SnippetKey(triggers:normalizedTriggers($0.triggers),content:Data($0.content.utf8),formatting:$0.formatting.flatMap { try? KudzuJSON.encode($0,pretty:false) }) }
        var survivors: [Snippet] = []
        for bucket in buckets.values {
            let sorted = bucket.sorted { $0.createdAt != $1.createdAt ? $0.createdAt < $1.createdAt : $0.id.lowercased < $1.id.lowercased }
            guard var survivor = sorted.first else { continue }; let original = survivor
            for loser in sorted.dropFirst() {
                p.snippetMerges[loser.id] = survivor.id
                survivor.isFavorite = survivor.isFavorite || loser.isFavorite
                survivor.isEnabled = survivor.isEnabled || loser.isEnabled
                survivor.updatedAt = max(survivor.updatedAt,loser.updatedAt)
            }
            survivor.groupID = survivor.groupID.map(resolve)
            if survivor != original { p.updatedSnippets.append(survivor) }; survivors.append(survivor)
        }
        p.updatedSnippets.sort { $0.id.lowercased < $1.id.lowercased }
        var claims: [Data:Set<UUID>] = [:]
        for s in survivors { for t in normalizedTriggers(s.triggers) { claims[Data(t.utf8),default:[]].insert(s.id) } }
        p.conflicts = claims.keys.sorted { $0.lexicographicallyPrecedes($1) }.compactMap { t in
            guard let ids = claims[t], ids.count > 1 else { return nil }
            return Conflict(trigger:String(decoding:t,as:UTF8.self),snippetIds:ids.sorted { $0.lowercased < $1.lowercased })
        }
        func canonical(_ id: UUID) -> UUID {
            var cur = id, seen: Set<UUID> = []
            while snippets[cur] == nil, let next = aliases[cur], seen.insert(cur).inserted { cur = next }
            if snippets[cur] == nil { return id }; return p.snippetMerges[cur] ?? cur
        }
        let usage = Dictionary(grouping:library.usage) { UsageKey(id:canonical($0.snippetID),device:Data($0.deviceID.utf8)) }
        for (key,rows) in usage where rows.count > 1 || rows[0].snippetID != key.id {
            let device = String(decoding:key.device,as:UTF8.self)
            p.usageMerges.append(UsageMerge(snippetID:key.id,deviceID:device,sourceSnippetIDs:Set(rows.map(\.snippetID)).sorted { $0.lowercased < $1.lowercased },count:rows.reduce(0) { $0+$1.count },charsSaved:rows.reduce(0) { $0+$1.charsSaved },lastUsedAt:rows.compactMap(\.lastUsedAt).max()))
        }
        p.usageMerges.sort { $0.snippetID != $1.snippetID ? $0.snippetID.lowercased < $1.snippetID.lowercased : $0.deviceID.utf8.lexicographicallyPrecedes($1.deviceID.utf8) }
        return p
    }
    /// Plans and applies a dedupe pass, returning the cleaned library and what changed.
    public static func dedupe(_ library: Library, aliases: [UUID:UUID] = [:]) -> (library: Library, report: Report) {
        let p = plan(library,aliases:aliases); return (p.applied(library),p.report)
    }
}
