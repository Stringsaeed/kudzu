import Foundation

/// Immutable lookup structure answering "did the last keystroke complete a trigger?".
///
/// Built from the library plus a `MatchingConfig`; rebuild it (cheaply, off the event-tap thread)
/// whenever snippets, groups or settings change. Matching walks reversed-trigger tries backwards
/// from the end of the buffer, so a keystroke costs O(longest trigger), independent of snippet count.
public struct TriggerIndex: Sendable {
    /// An index with no triggers.
    public static let empty = TriggerIndex(snippets: [], groups: [], config: MatchingConfig())

    public let config: MatchingConfig

    private var snippets: [IndexedSnippet] = []
    private var entries: [Entry] = []
    /// Case-insensitive triggers, keyed by case-folded characters.
    private var folded = SuffixTrie()
    /// Case-sensitive triggers, keyed by the characters as written.
    private var exact = SuffixTrie()

    /// Number of (snippet, trigger) pairs that can fire.
    public var triggerCount: Int { entries.count }

    /// Builds the index. Snippets that are effectively disabled (themselves or via any ancestor
    /// group) and snippets without a non-empty trigger are left out. A snippet whose `groupID`
    /// names an unknown group is treated as ungrouped.
    public init(snippets: [Snippet], groups: [Group], config: MatchingConfig) {
        self.config = config
        let resolver = GroupResolver(groups: groups)

        for snippet in snippets {
            let effective = EffectiveSnippet(snippet, groups: resolver)
            guard effective.isEnabled else { continue }

            var seen: Set<String> = []
            var triggers: [(text: String, chars: [Character])] = []
            for trigger in snippet.triggers where !trigger.isEmpty {
                let key = (snippet.caseMode == .propagate ? TriggerFolding.fold(trigger) : trigger)
                if seen.insert(key).inserted { triggers.append((trigger, Array(trigger))) }
            }
            guard !triggers.isEmpty else { continue }

            let snippetIndex = Int32(self.snippets.count)
            self.snippets.append(
                IndexedSnippet(
                    id: snippet.id,
                    updatedAt: snippet.updatedAt,
                    caseMode: snippet.caseMode,
                    rule: effective.rule.map(IndexedRule.init),
                    insertion: effective.insertion
                )
            )
            for (text, chars) in triggers {
                let entryIndex = Int32(entries.count)
                entries.append(Entry(snippet: snippetIndex, trigger: text, length: chars.count, fireMode: snippet.fireMode))
                if snippet.caseMode == .propagate {
                    folded.insert(chars.map(TriggerFolding.fold), entry: entryIndex)
                } else {
                    exact.insert(chars, entry: entryIndex)
                }
            }
        }
    }

    /// Returns the trigger completed by the most recent keystroke, if any.
    ///
    /// `buffer` must already include the keystroke described by `lastInput`; only
    /// `.character` inputs can fire a trigger (a backspace or reset never does, even if the
    /// remaining buffer happens to end with a trigger).
    ///
    /// - `.immediate` snippets fire when the buffer ends with the trigger.
    /// - `.wholeWord` snippets fire when the last character is a separator, the characters before
    ///   it end with the trigger, and the character before the trigger is a separator or the start of the buffer.
    ///
    /// When several triggers match, the longest wins, then the most recently updated snippet.
    ///
    /// Insertion method precedence for a matched snippet:
    /// 1. The matching snippet/group `AppRule.insertion` (when `bundleID` is listed in `rule.bundleIDs`).
    /// 2. `snippet.insertion`.
    /// 3. The nearest ancestor group's `insertion`.
    /// 4. The global per-app override in `config.appInsertions[bundleID]`.
    /// 5. `config.defaultInsertion`.
    public func match(buffer: KeyBuffer, lastInput: KeyInput, bundleID: String?) -> TriggerMatch? {
        guard case .character = lastInput, !entries.isEmpty else { return nil }
        let chars = buffer.characters
        let count = chars.count
        guard count > 0 else { return nil }
        if let bundleID, config.deniedBundleIDs.contains(bundleID) { return nil }

        var best: Candidate?
        search(chars, end: count, wholeWord: false, bundleID: bundleID, best: &best)
        if count >= 2, config.isSeparator(chars[count - 1]) {
            search(chars, end: count - 1, wholeWord: true, bundleID: bundleID, best: &best)
        }
        guard let best else { return nil }

        let entry = entries[Int(best.entry)]
        let snippet = snippets[Int(entry.snippet)]
        let typed = String(chars[best.start..<best.end])
        let separator = entry.fireMode == .wholeWord ? String(chars[count - 1]) : nil
        return TriggerMatch(
            snippetID: snippet.id,
            trigger: entry.trigger,
            typedTrigger: typed,
            deleteCount: entry.length,
            separator: separator,
            insertion: snippet.insertion(for: bundleID, config: config),
            caseStyle: snippet.caseMode == .propagate ? CasePropagation.detect(typedTrigger: typed) : .asIs,
            fireMode: entry.fireMode
        )
    }

    // MARK: - Search

    private struct Candidate {
        var entry: Int32
        var start: Int
        var end: Int
    }

    /// Walks both tries backwards from `chars[end - 1]`, considering every trigger that ends at `end`.
    private func search(
        _ chars: ContiguousArray<Character>,
        end: Int,
        wholeWord: Bool,
        bundleID: String?,
        best: inout Candidate?
    ) {
        var foldedNode: Int32? = 0
        var exactNode: Int32? = 0
        var depth = 0
        while depth < end, foldedNode != nil || exactNode != nil {
            let character = chars[end - 1 - depth]
            if let node = foldedNode {
                foldedNode = folded.child(of: node, TriggerFolding.fold(character))
            }
            if let node = exactNode {
                exactNode = exact.child(of: node, character)
            }
            depth += 1
            let start = end - depth
            if let node = foldedNode {
                consider(folded.entries(at: node), start: start, end: end, chars: chars, wholeWord: wholeWord, bundleID: bundleID, best: &best)
            }
            if let node = exactNode {
                consider(exact.entries(at: node), start: start, end: end, chars: chars, wholeWord: wholeWord, bundleID: bundleID, best: &best)
            }
        }
    }

    private func consider(
        _ candidates: [Int32],
        start: Int,
        end: Int,
        chars: ContiguousArray<Character>,
        wholeWord: Bool,
        bundleID: String?,
        best: inout Candidate?
    ) {
        for entryIndex in candidates {
            let entry = entries[Int(entryIndex)]
            guard (entry.fireMode == .wholeWord) == wholeWord else { continue }
            if wholeWord, start > 0, !config.isSeparator(chars[start - 1]) { continue }
            let snippet = snippets[Int(entry.snippet)]
            if let rule = snippet.rule, !rule.permits(bundleID) { continue }
            if let current = best, !outranks(entryIndex, current.entry) { continue }
            best = Candidate(entry: entryIndex, start: start, end: end)
        }
    }

    /// Longest trigger, then most recently updated, then earliest in the input.
    private func outranks(_ lhs: Int32, _ rhs: Int32) -> Bool {
        let l = entries[Int(lhs)], r = entries[Int(rhs)]
        if l.length != r.length { return l.length > r.length }
        let lDate = snippets[Int(l.snippet)].updatedAt, rDate = snippets[Int(r.snippet)].updatedAt
        if lDate != rDate { return lDate > rDate }
        return lhs < rhs
    }
}

// MARK: - Storage

private struct Entry: Sendable {
    var snippet: Int32
    var trigger: String
    var length: Int
    var fireMode: FireMode
}

private struct IndexedRule: Sendable {
    var mode: AppRuleMode
    var bundleIDs: Set<String>
    var insertion: InsertionMethod?

    init(_ rule: AppRule) {
        mode = rule.mode
        bundleIDs = Set(rule.bundleIDs)
        insertion = rule.insertion
    }

    /// An unknown app (`nil`) cannot satisfy an allow-list and is not caught by a deny-list.
    func permits(_ bundleID: String?) -> Bool {
        switch mode {
        case .allow:
            guard let bundleID else { return false }
            return bundleIDs.contains(bundleID)
        case .deny:
            guard let bundleID else { return true }
            return !bundleIDs.contains(bundleID)
        }
    }
}

private struct IndexedSnippet: Sendable {
    var id: UUID
    var updatedAt: Date
    var caseMode: CaseMode
    var rule: IndexedRule?
    /// `snippet.insertion` ▸ nearest group's `insertion`, before any per-app or config fallback.
    var insertion: InsertionMethod?

    /// Resolves the insertion method for `bundleID` in precedence order:
    /// 1. The matching snippet/group `AppRule.insertion`, when `bundleID` is in the rule's `bundleIDs`.
    /// 2. `snippet.insertion`.
    /// 3. The nearest ancestor group's `insertion`.
    /// 4. The global per-app override in `config.appInsertions[bundleID]`.
    /// 5. `config.defaultInsertion`.
    func insertion(for bundleID: String?, config: MatchingConfig) -> InsertionMethod {
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

/// Trie over reversed keys: the path from the root spells a trigger from its last character backwards.
private struct SuffixTrie: Sendable {
    private struct Node: Sendable {
        var children: [Character: Int32] = [:]
        var entries: [Int32] = []
    }

    private var nodes: [Node] = [Node()]

    mutating func insert(_ key: [Character], entry: Int32) {
        var current = 0
        for character in key.reversed() {
            if let next = nodes[current].children[character] {
                current = Int(next)
            } else {
                nodes.append(Node())
                let next = nodes.count - 1
                nodes[current].children[character] = Int32(next)
                current = next
            }
        }
        nodes[current].entries.append(entry)
    }

    func child(of node: Int32, _ character: Character) -> Int32? {
        nodes[Int(node)].children[character]
    }

    func entries(at node: Int32) -> [Int32] {
        nodes[Int(node)].entries
    }
}

extension TriggerIndex {
    /// The trigger completed by an input already applied to the buffer.
    public func findMatch(buffer: KeyBuffer, lastInput: KeyInput, bundleID: String? = nil) -> TriggerMatch? {
        match(buffer: buffer, lastInput: lastInput, bundleID: bundleID)
    }
}
