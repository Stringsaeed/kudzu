import Foundation

extension InsertionMethod {
    /// Human-readable display title for menus and descriptions.
    public var title: String {
        switch self {
        case .automatic: "Automatic"
        case .type: "Type"
        case .paste: "Paste"
        }
    }
}

extension AppRule {
    /// Human-readable summary of this rule, e.g. "Block in Slack, Mail" or "Allow only in Terminal".
    public func summary(appName: (String) -> String = { $0 }) -> String {
        if bundleIDs.isEmpty {
            return mode == .allow ? "Disabled everywhere" : "No restrictions"
        }
        let names = bundleIDs.map(appName).joined(separator: ", ")
        switch mode {
        case .allow: return "Allow only in \(names)"
        case .deny: return "Block in \(names)"
        }
    }
}

/// Settings a snippet or subgroup inherits from its group ancestry.
public struct InheritedGroupSettings: Hashable, Sendable {
    public var isEnabled: Bool
    public var rule: AppRule?
    /// The nearest group in the ancestry that defined `rule`.
    public var ruleGroup: Group?
    public var insertion: InsertionMethod?
    /// The nearest group in the ancestry that defined `insertion`.
    public var insertionGroup: Group?

    public init(
        isEnabled: Bool = true,
        rule: AppRule? = nil,
        ruleGroup: Group? = nil,
        insertion: InsertionMethod? = nil,
        insertionGroup: Group? = nil
    ) {
        self.isEnabled = isEnabled
        self.rule = rule
        self.ruleGroup = ruleGroup
        self.insertion = insertion
        self.insertionGroup = insertionGroup
    }

    /// Describes the inherited app rule and the group it comes from, or `"No restrictions"`.
    public func ruleDescription(appName: (String) -> String = { $0 }) -> String {
        guard let rule else { return "No restrictions" }
        let summary = rule.summary(appName: appName)
        if let group = ruleGroup {
            return "Inherited from group '\(group.name)': \(summary)"
        }
        return summary
    }

    /// Describes the inherited insertion method and the group it comes from, or the global default.
    public func insertionDescription(defaultInsertion: InsertionMethod = .automatic) -> String {
        if let insertion, let group = insertionGroup {
            return "Inherited from group '\(group.name)': \(insertion.title)"
        }
        if let insertion {
            return insertion.title
        }
        return "Default (\(defaultInsertion.title), or the app's setting in Settings → Apps)"
    }
}

/// Resolves enablement, app rule and insertion through the group tree (`parentID`), tolerating
/// cycles and dangling parent IDs.
public struct GroupResolver: Sendable {
    private var byGroup: [UUID: InheritedGroupSettings] = [:]
    private var parentSettingsByGroup: [UUID: InheritedGroupSettings] = [:]

    public init(groups: [Group]) {
        var records: [UUID: Group] = [:]
        for group in groups where records[group.id] == nil { records[group.id] = group }
        for group in records.values {
            byGroup[group.id] = Self.walk(from: group, excluding: nil, in: records)
            parentSettingsByGroup[group.id] = Self.walk(
                from: group.parentID.flatMap { records[$0] },
                excluding: group.id,
                in: records
            )
        }
    }

    /// Settings inherited by a snippet whose group is `groupID` (nil or unknown → none).
    public func settings(forGroup groupID: UUID?) -> InheritedGroupSettings {
        groupID.flatMap { byGroup[$0] } ?? InheritedGroupSettings()
    }

    /// Settings inherited by `groupID` itself from its parent chain (excluding `groupID`'s own settings).
    public func inheritedSettings(forGroupItself groupID: UUID) -> InheritedGroupSettings {
        parentSettingsByGroup[groupID] ?? InheritedGroupSettings()
    }

    /// The nearest rule set on `groupID` or one of its ancestors, with the group that sets it.
    public func inheritedRule(forGroup groupID: UUID?) -> (rule: AppRule, from: Group)? {
        let s = settings(forGroup: groupID)
        guard let rule = s.rule, let from = s.ruleGroup else { return nil }
        return (rule, from)
    }

    /// The nearest insertion override on `groupID` or one of its ancestors, with the group that sets it.
    public func inheritedInsertion(forGroup groupID: UUID?) -> (method: InsertionMethod, from: Group)? {
        let s = settings(forGroup: groupID)
        guard let method = s.insertion, let from = s.insertionGroup else { return nil }
        return (method, from)
    }

    private static func walk(
        from start: Group?,
        excluding excludedID: UUID?,
        in records: [UUID: Group]
    ) -> InheritedGroupSettings {
        var result = InheritedGroupSettings()
        var visited: Set<UUID> = excludedID.map { [$0] } ?? []
        var current: Group? = start
        // Walk child → root so the nearest ancestor wins for rule and insertion, while
        // any disabled ancestor disables the whole subtree.
        while let group = current, visited.insert(group.id).inserted {
            if !group.isEnabled { result.isEnabled = false }
            if result.rule == nil, let rule = group.rule {
                result.rule = rule
                result.ruleGroup = group
            }
            if result.insertion == nil, let insertion = group.insertion {
                result.insertion = insertion
                result.insertionGroup = group
            }
            current = group.parentID.flatMap { records[$0] }
        }
        return result
    }
}

/// A snippet's effective, inheritance-resolved matching settings.
public struct EffectiveSnippet: Hashable, Sendable {
    public var isEnabled: Bool
    public var rule: AppRule?
    /// The group from which `rule` was inherited, or `nil` if the snippet sets its own rule (or has none).
    public var ruleGroup: Group?
    /// `snippet.insertion` ▸ nearest ancestor group's `insertion`; `nil` means fall back to global app/default.
    public var insertion: InsertionMethod?
    /// The group from which `insertion` was inherited, or `nil` if the snippet sets its own insertion (or has none).
    public var insertionGroup: Group?

    public init(_ snippet: Snippet, groups: GroupResolver) {
        let inherited = groups.settings(forGroup: snippet.groupID)
        isEnabled = snippet.isEnabled && inherited.isEnabled
        if let ownRule = snippet.rule {
            rule = ownRule
            ruleGroup = nil
        } else {
            rule = inherited.rule
            ruleGroup = inherited.ruleGroup
        }
        if let ownInsertion = snippet.insertion {
            insertion = ownInsertion
            insertionGroup = nil
        } else {
            insertion = inherited.insertion
            insertionGroup = inherited.insertionGroup
        }
    }

    /// Resolves the insertion method for `bundleID` in precedence order:
    /// 1. The effective (snippet or inherited group) `AppRule.insertion`, when `bundleID` is in the rule's `bundleIDs`.
    /// 2. `snippet.insertion`.
    /// 3. The nearest ancestor group's `insertion`.
    /// 4. The global per-app override in `config.appInsertions[bundleID]`.
    /// 5. `config.defaultInsertion`.
    public func insertion(for bundleID: String?, config: MatchingConfig) -> InsertionMethod {
        if let override = rule?.insertion, let bundleID, rule?.bundleIDs.contains(bundleID) == true {
            return override
        }
        if let insertion {
            return insertion
        }
        if let bundleID, let appOverride = config.appInsertions[bundleID] {
            return appOverride
        }
        return config.defaultInsertion
    }
}
