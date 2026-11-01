import Foundation

/// Two or more surviving snippets share a trigger but expand to different content.
public struct DedupeConflict: Hashable, Sendable {
    /// The shared trigger, normalized (lowercased).
    public var trigger: String
    /// The snippets claiming it, sorted by UUID string.
    public var snippetIDs: [UUID]

    public init(trigger: String, snippetIDs: [UUID]) {
        self.trigger = trigger
        self.snippetIDs = snippetIDs
    }
}

/// Collapse every usage row for `deviceID` whose snippet is in `sourceSnippetIDs` into one row for `snippetID`.
public struct UsageMerge: Hashable, Sendable {
    public var snippetID: UUID
    public var deviceID: String
    /// Snippet IDs whose rows (for this device) are folded in, sorted by UUID string. May include `snippetID`.
    public var sourceSnippetIDs: [UUID]
    public var count: Int
    public var charsSaved: Int
    public var lastUsedAt: Date?

    public init(snippetID: UUID, deviceID: String, sourceSnippetIDs: [UUID], count: Int, charsSaved: Int, lastUsedAt: Date?) {
        self.snippetID = snippetID
        self.deviceID = deviceID
        self.sourceSnippetIDs = sourceSnippetIDs
        self.count = count
        self.charsSaved = charsSaved
        self.lastUsedAt = lastUsedAt
    }

    /// The single usage row this merge produces.
    public var record: UsageRecord {
        UsageRecord(snippetID: snippetID, deviceID: deviceID, count: count, charsSaved: charsSaved, lastUsedAt: lastUsedAt)
    }

    func covers(_ row: UsageRecord) -> Bool {
        row.deviceID == deviceID && sourceSnippetIDs.contains(row.snippetID)
    }
}

/// The changes a dedupe pass wants to make. Pure data; `KudzuStore` applies it to SwiftData.
public struct DedupePlan: Hashable, Sendable {
    /// Loser group ID → surviving group ID.
    public var groupMerges: [UUID: UUID] = [:]
    /// Loser snippet ID → surviving snippet ID.
    public var snippetMerges: [UUID: UUID] = [:]
    /// Surviving groups whose fields changed (re-parented), sorted by UUID string.
    public var updatedGroups: [GroupRecord] = []
    /// Surviving snippets whose fields changed (merged flags, re-grouped), sorted by UUID string.
    public var updatedSnippets: [SnippetRecord] = []
    /// Usage rows to collapse, sorted by snippet then device.
    public var usageMerges: [UsageMerge] = []
    /// Same trigger, different content. Informational: not resolved by the plan.
    public var conflicts: [DedupeConflict] = []

    public init() {}

    /// True when applying the plan would change nothing (conflicts don't count).
    public var isEmpty: Bool {
        groupMerges.isEmpty && snippetMerges.isEmpty && updatedGroups.isEmpty && updatedSnippets.isEmpty && usageMerges.isEmpty
    }

    /// The library after applying the plan. Mirrors what `KudzuStore` does to the persistent store.
    public func applied(to library: KudzuLibrary) -> KudzuLibrary {
        guard !isEmpty else { return library }
        var result = library
        let groupUpdates = Dictionary(updatedGroups.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        result.groups = library.groups
            .filter { groupMerges[$0.id] == nil }
            .map { groupUpdates[$0.id] ?? $0 }
        let snippetUpdates = Dictionary(updatedSnippets.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        result.snippets = library.snippets
            .filter { snippetMerges[$0.id] == nil }
            .map { snippetUpdates[$0.id] ?? $0 }
        result.usage = library.usage.filter { row in !usageMerges.contains { $0.covers(row) } }
            + usageMerges.map(\.record)
        return result
    }
}

/// Finds duplicates created by syncing or repeated imports. Pure, deterministic and idempotent.
///
/// Every device runs this independently after remote changes, so the
/// survivor choice must depend only on the data (never on fetch order or the
/// device) for all devices to converge on the same result.
public enum Deduplicator {
    /// Triggers as a set: lowercased, de-duplicated and sorted.
    public static func normalizedTriggers(_ triggers: [String]) -> [String] {
        Array(Set(triggers.map { $0.lowercased() })).sorted()
    }

    /// Computes the merge plan for `library`.
    ///
    /// - Parameter aliases: former snippet ID → the snippet it was merged into
    ///   by an earlier pass (possibly on another device). Used to re-point usage
    ///   rows that arrived after their snippet had already been merged away.
    public static func plan(for library: KudzuLibrary, aliases: [UUID: UUID] = [:]) -> DedupePlan {
        var plan = DedupePlan()

        // Groups. Merging two parents can make their children siblings with
        // equal names, so repeat until nothing changes.
        let groupsByID = Dictionary(library.groups.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        var groupMerges: [UUID: UUID] = [:]
        func resolveGroup(_ id: UUID) -> UUID {
            var current = id
            while let next = groupMerges[current] { current = next }
            return current
        }
        struct GroupKey: Hashable { var parent: UUID?; var name: String }
        while true {
            var buckets: [GroupKey: [UUID]] = [:]
            for group in groupsByID.values where groupMerges[group.id] == nil {
                let key = GroupKey(parent: group.parentID.map(resolveGroup), name: normalizedGroupName(group.name))
                buckets[key, default: []].append(group.id)
            }
            var changed = false
            for ids in buckets.values where ids.count > 1 {
                let sorted = ids.sorted(by: uuidLess)
                for loser in sorted.dropFirst() {
                    groupMerges[loser] = sorted[0]
                    changed = true
                }
            }
            if !changed { break }
        }
        plan.groupMerges = groupMerges.mapValues(resolveGroup)
        plan.updatedGroups = groupsByID.values
            .filter { groupMerges[$0.id] == nil }
            .compactMap { group in
                let parent = group.parentID.map(resolveGroup)
                guard parent != group.parentID else { return nil }
                var updated = group
                updated.parentID = parent
                return updated
            }
            .sorted { uuidLess($0.id, $1.id) }

        // Snippets: identical normalized trigger set and content.
        struct SnippetKey: Hashable { var triggers: [String]; var content: String }
        let snippetsByID = Dictionary(library.snippets.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        var snippetBuckets: [SnippetKey: [SnippetRecord]] = [:]
        for snippet in snippetsByID.values {
            snippetBuckets[SnippetKey(triggers: normalizedTriggers(snippet.triggers), content: snippet.content), default: []].append(snippet)
        }
        var survivors: [SnippetRecord] = []
        for bucket in snippetBuckets.values {
            let sorted = bucket.sorted { lhs, rhs in
                lhs.createdAt != rhs.createdAt ? lhs.createdAt < rhs.createdAt : uuidLess(lhs.id, rhs.id)
            }
            var survivor = sorted[0]
            let original = survivor
            for loser in sorted.dropFirst() {
                plan.snippetMerges[loser.id] = survivor.id
                survivor.isFavorite = survivor.isFavorite || loser.isFavorite
                survivor.isEnabled = survivor.isEnabled || loser.isEnabled
                survivor.updatedAt = max(survivor.updatedAt, loser.updatedAt)
            }
            survivor.groupID = survivor.groupID.map(resolveGroup)
            if survivor != original { plan.updatedSnippets.append(survivor) }
            survivors.append(survivor)
        }
        plan.updatedSnippets.sort { uuidLess($0.id, $1.id) }

        // Conflicts among survivors: a trigger claimed by different content.
        var claims: [String: Set<UUID>] = [:]
        for snippet in survivors {
            for trigger in normalizedTriggers(snippet.triggers) {
                claims[trigger, default: []].insert(snippet.id)
            }
        }
        plan.conflicts = claims
            .filter { $0.value.count > 1 }
            .map { DedupeConflict(trigger: $0.key, snippetIDs: $0.value.sorted(by: uuidLess)) }
            .sorted { $0.trigger < $1.trigger }

        // Usage: re-point to survivors and fold rows sharing (snippet, device).
        let liveSnippetIDs = Set(snippetsByID.keys)
        func canonicalSnippet(_ id: UUID) -> UUID {
            var current = id
            var seen: Set<UUID> = []
            while !liveSnippetIDs.contains(current), let next = aliases[current], seen.insert(current).inserted {
                current = next
            }
            // An orphan row (snippet not synced yet, or alias to nowhere) stays as is.
            guard liveSnippetIDs.contains(current) else { return id }
            return plan.snippetMerges[current] ?? current
        }
        struct UsageKey: Hashable { var snippet: UUID; var device: String }
        var usageBuckets: [UsageKey: [UsageRecord]] = [:]
        for row in library.usage {
            usageBuckets[UsageKey(snippet: canonicalSnippet(row.snippetID), device: row.deviceID), default: []].append(row)
        }
        for (key, rows) in usageBuckets where rows.count > 1 || rows[0].snippetID != key.snippet {
            plan.usageMerges.append(UsageMerge(
                snippetID: key.snippet,
                deviceID: key.device,
                sourceSnippetIDs: Set(rows.map(\.snippetID)).sorted(by: uuidLess),
                count: rows.reduce(0) { $0 + $1.count },
                charsSaved: rows.reduce(0) { $0 + $1.charsSaved },
                lastUsedAt: rows.reduce(nil) { StatsCalculator.latest($0, $1.lastUsedAt) }
            ))
        }
        plan.usageMerges.sort { lhs, rhs in
            lhs.snippetID != rhs.snippetID ? uuidLess(lhs.snippetID, rhs.snippetID) : lhs.deviceID < rhs.deviceID
        }
        return plan
    }

    static func normalizedGroupName(_ name: String) -> String {
        name.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    }

    /// The tie-break order used everywhere: UUID strings compared lexicographically.
    static func uuidLess(_ lhs: UUID, _ rhs: UUID) -> Bool {
        lhs.uuidString < rhs.uuidString
    }
}
