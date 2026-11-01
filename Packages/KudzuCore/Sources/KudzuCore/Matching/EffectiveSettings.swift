import Foundation

/// Settings a snippet inherits from its group ancestry.
struct InheritedGroupSettings: Sendable {
    var isEnabled = true
    var rule: AppRule?
    var insertion: InsertionMethod?
}

/// Resolves enablement, app rule and insertion through the group tree (`parentID`), tolerating
/// cycles and dangling parent IDs.
struct GroupResolver: Sendable {
    private var byGroup: [UUID: InheritedGroupSettings] = [:]

    init(groups: [GroupRecord]) {
        var records: [UUID: GroupRecord] = [:]
        for group in groups where records[group.id] == nil { records[group.id] = group }
        for group in records.values {
            byGroup[group.id] = Self.walk(from: group, in: records)
        }
    }

    /// Settings inherited by something whose parent group is `groupID` (nil or unknown → none).
    func settings(forGroup groupID: UUID?) -> InheritedGroupSettings {
        groupID.flatMap { byGroup[$0] } ?? InheritedGroupSettings()
    }

    // Walks self → root. Every visited group must be enabled; the first non-nil rule/insertion wins.
    // A revisited group (cycle) ends the walk, so each group in a cycle is still counted once.
    private static func walk(from start: GroupRecord, in records: [UUID: GroupRecord]) -> InheritedGroupSettings {
        var result = InheritedGroupSettings()
        var visited: Set<UUID> = []
        var current: GroupRecord? = start
        while let group = current, visited.insert(group.id).inserted {
            if !group.isEnabled { result.isEnabled = false }
            if result.rule == nil { result.rule = group.rule }
            if result.insertion == nil { result.insertion = group.insertion }
            current = group.parentID.flatMap { records[$0] }
        }
        return result
    }
}

/// A snippet's effective, inheritance-resolved matching settings.
struct EffectiveSnippet: Sendable {
    var isEnabled: Bool
    var rule: AppRule?
    /// snippet ▸ nearest group; `nil` means "use the config default".
    var insertion: InsertionMethod?

    init(_ snippet: SnippetRecord, groups: GroupResolver) {
        let inherited = groups.settings(forGroup: snippet.groupID)
        isEnabled = snippet.isEnabled && inherited.isEnabled
        rule = snippet.rule ?? inherited.rule
        insertion = snippet.insertion ?? inherited.insertion
    }
}
