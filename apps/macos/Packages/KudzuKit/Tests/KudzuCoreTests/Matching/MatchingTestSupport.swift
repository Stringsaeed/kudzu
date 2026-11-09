import Foundation
@testable import KudzuCore

/// Builds a snippet with only the fields matching cares about.
func snip(
    _ triggers: [String],
    fire: FireMode = .immediate,
    case caseMode: CaseMode = .propagate,
    group: UUID? = nil,
    insertion: InsertionMethod? = nil,
    rule: AppRule? = nil,
    enabled: Bool = true,
    updated: TimeInterval = 0,
    id: UUID = UUID()
) -> Snippet {
    Snippet(
        id: id,
        name: triggers.first ?? "",
        triggers: triggers,
        content: "content",
        groupID: group,
        fireMode: fire,
        caseMode: caseMode,
        insertion: insertion,
        rule: rule,
        isEnabled: enabled,
        createdAt: Date(timeIntervalSince1970: 0),
        updatedAt: Date(timeIntervalSince1970: updated)
    )
}

func group(
    _ name: String = "g",
    parent: UUID? = nil,
    enabled: Bool = true,
    insertion: InsertionMethod? = nil,
    rule: AppRule? = nil,
    id: UUID = UUID()
) -> Group {
    Group(id: id, name: name, parentID: parent, isEnabled: enabled, insertion: insertion, rule: rule)
}

/// Types `text` one character at a time and returns the match produced by each keystroke.
func typeAll(
    _ text: String,
    on index: TriggerIndex,
    bundleID: String? = nil
) -> [TriggerMatch?] {
    var buffer = KeyBuffer()
    var results: [TriggerMatch?] = []
    for ch in text {
        let input = KeyInput.character(String(ch))
        buffer.apply(input)
        results.append(index.match(buffer: buffer, lastInput: input, bundleID: bundleID))
    }
    return results
}

/// Types `text` and returns the match produced by the final keystroke only.
func type(_ text: String, on index: TriggerIndex, bundleID: String? = nil) -> TriggerMatch? {
    typeAll(text, on: index, bundleID: bundleID).last ?? nil
}

/// Types `text` and returns the first match produced by any keystroke.
func firstMatch(_ text: String, on index: TriggerIndex, bundleID: String? = nil) -> TriggerMatch? {
    typeAll(text, on: index, bundleID: bundleID).compactMap { $0 }.first
}

func makeIndex(
    _ snippets: [Snippet],
    groups: [Group] = [],
    config: MatchingConfig = MatchingConfig()
) -> TriggerIndex {
    TriggerIndex(snippets: snippets, groups: groups, config: config)
}
