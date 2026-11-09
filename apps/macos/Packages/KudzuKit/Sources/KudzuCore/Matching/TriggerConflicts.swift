import Foundation

/// A problem between two snippets' triggers, for editor warnings.
public struct TriggerConflict: Hashable, Sendable {
    public enum Kind: Hashable, Sendable {
        /// Both snippets use the same trigger (case-insensitively if either propagates case).
        case duplicate
        /// `trigger` is an `.immediate` trigger and a proper prefix of `otherTrigger`,
        /// so `otherTrigger` can never be typed in full: `trigger` fires first.
        case shadowing
    }

    public let kind: Kind
    /// For `.shadowing`, the shadowing snippet (the one with the shorter, immediate trigger).
    public let snippetID: UUID
    public let trigger: String
    /// For `.shadowing`, the shadowed snippet.
    public let otherSnippetID: UUID
    public let otherTrigger: String

    public init(kind: Kind, snippetID: UUID, trigger: String, otherSnippetID: UUID, otherTrigger: String) {
        self.kind = kind
        self.snippetID = snippetID
        self.trigger = trigger
        self.otherSnippetID = otherSnippetID
        self.otherTrigger = otherTrigger
    }
}

public enum TriggerConflicts {
    /// Finds duplicate and shadowed triggers among effectively enabled snippets.
    ///
    /// Snippets whose app rules can never apply to the same app (e.g. disjoint allow-lists) do not
    /// conflict. Two triggers on the same snippet never conflict. An `.exact` shadower is only
    /// reported against an `.exact` longer trigger, or when the shared prefix has no letter case.
    public static func find(snippets: [Snippet], groups: [Group]) -> [TriggerConflict] {
        let resolver = GroupResolver(groups: groups)
        var items: [Item] = []
        for snippet in snippets {
            let effective = EffectiveSnippet(snippet, groups: resolver)
            guard effective.isEnabled else { continue }
            for trigger in snippet.triggers where !trigger.isEmpty {
                items.append(Item(
                    id: snippet.id,
                    trigger: trigger,
                    chars: Array(trigger),
                    propagates: snippet.caseMode == .propagate,
                    immediate: snippet.fireMode == .immediate,
                    rule: effective.rule
                ))
            }
        }

        var conflicts = duplicates(in: items)
        conflicts += shadowing(in: items)
        return conflicts
    }

    // MARK: - Duplicates

    private static func duplicates(in items: [Item]) -> [TriggerConflict] {
        var buckets: [String: [Int]] = [:]
        for (i, item) in items.enumerated() {
            buckets[TriggerFolding.fold(item.trigger), default: []].append(i)
        }
        var found: [(Int, Int)] = []
        for indices in buckets.values where indices.count > 1 {
            for (n, i) in indices.enumerated() {
                for j in indices[(n + 1)...] {
                    let a = items[i], b = items[j]
                    guard a.id != b.id, a.propagates || b.propagates || a.trigger == b.trigger else { continue }
                    guard rulesCanOverlap(a.rule, b.rule) else { continue }
                    found.append((min(i, j), max(i, j)))
                }
            }
        }
        return found.sorted { ($0.0, $0.1) < ($1.0, $1.1) }.map { i, j in
            TriggerConflict(kind: .duplicate, snippetID: items[i].id, trigger: items[i].trigger,
                            otherSnippetID: items[j].id, otherTrigger: items[j].trigger)
        }
    }

    // MARK: - Shadowing

    private static func shadowing(in items: [Item]) -> [TriggerConflict] {
        var foldedShadowers: [String: [Int]] = [:]
        var exactShadowers: [String: [Int]] = [:]
        for (i, item) in items.enumerated() where item.immediate {
            if item.propagates {
                foldedShadowers[TriggerFolding.fold(item.trigger), default: []].append(i)
            } else {
                exactShadowers[item.trigger, default: []].append(i)
            }
        }

        var found: [(shadower: Int, shadowed: Int)] = []
        for (j, longer) in items.enumerated() where longer.chars.count > 1 {
            for length in 1..<longer.chars.count {
                let prefix = Array(longer.chars[..<length])
                let foldedPrefix = String(prefix.map(TriggerFolding.fold))
                let rawPrefix = String(prefix)
                // A propagating shadower fires whatever case the longer trigger is typed in.
                for i in foldedShadowers[foldedPrefix] ?? [] { found.append((i, j)) }
                // An exact shadower only fires on its exact text: it blocks an exact longer
                // trigger, or a propagating one whose prefix cannot be typed in any other case.
                if longer.propagates && prefix.contains(where: \.isCased) { continue }
                for i in exactShadowers[rawPrefix] ?? [] { found.append((i, j)) }
            }
        }
        return found
            .filter { items[$0.shadower].id != items[$0.shadowed].id }
            .filter { rulesCanOverlap(items[$0.shadower].rule, items[$0.shadowed].rule) }
            .sorted { ($0.shadowed, $0.shadower) < ($1.shadowed, $1.shadower) }
            .map { pair in
                let a = items[pair.shadower], b = items[pair.shadowed]
                return TriggerConflict(kind: .shadowing, snippetID: a.id, trigger: a.trigger,
                                       otherSnippetID: b.id, otherTrigger: b.trigger)
            }
    }

    // MARK: - Rule overlap

    /// Whether some app exists where both rules allow expansion. No rule means every app.
    private static func rulesCanOverlap(_ a: AppRule?, _ b: AppRule?) -> Bool {
        guard let a, let b else { return true }
        let aIDs = Set(a.bundleIDs), bIDs = Set(b.bundleIDs)
        switch (a.mode, b.mode) {
        case (.allow, .allow): return !aIDs.isDisjoint(with: bIDs)
        case (.allow, .deny): return !aIDs.isSubset(of: bIDs)
        case (.deny, .allow): return !bIDs.isSubset(of: aIDs)
        case (.deny, .deny): return true
        }
    }

    private struct Item {
        var id: UUID
        var trigger: String
        var chars: [Character]
        var propagates: Bool
        var immediate: Bool
        var rule: AppRule?
    }
}
