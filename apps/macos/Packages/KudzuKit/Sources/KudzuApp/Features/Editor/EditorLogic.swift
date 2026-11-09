import Foundation
import KudzuCore

enum EditorLogic {
    /// Nearest ancestor setting, including the snippet's own group. Malformed cycles terminate.
    static func inherited<Value>(_ groups: [SnippetGroup], groupID: UUID?, key: KeyPath<SnippetGroup, Value?>) -> (value: Value, from: SnippetGroup)? {
        var id = groupID
        var seen = Set<UUID>()
        while let current = id, seen.insert(current).inserted,
              let group = groups.first(where: { $0.id == current }) {
            if let value = group[keyPath: key] { return (value, group) }
            id = group.parentID
        }
        return nil
    }

    static func describeRule(_ rule: AppRule?, apps: [AppInfo]) -> String {
        guard let rule else { return "Works in every app" }
        if rule.bundleIDs.isEmpty { return rule.mode == .allow ? "Disabled everywhere" : "Works in every app" }
        let names = rule.bundleIDs.prefix(3).map { id in apps.first { $0.bundleId == id }?.name ?? id }.joined(separator: ", ")
            + (rule.bundleIDs.count > 3 ? " +\(rule.bundleIDs.count - 3)" : "")
        return rule.mode == .allow ? "Only in \(names)" : "Everywhere except \(names)"
    }

    static func conflictMessage(_ conflict: TriggerConflict, snippetID: UUID) -> String {
        let mine = conflict.snippetID == snippetID
        switch conflict.kind {
        case .duplicate: return "“\(mine ? conflict.trigger : conflict.otherTrigger)” is also used by"
        case .shadowing:
            return mine ? "“\(conflict.trigger)” fires before “\(conflict.otherTrigger)” can be finished in"
                : "“\(conflict.otherTrigger)” can't be typed in full — “\(conflict.trigger)” fires first in"
        }
    }
}

/// Editable attributes preserve unknown keys when a chip is configured.
struct EditorTagDraft: Equatable {
    var name: String
    var attrs: [String: AttributeValue]
    init(_ tag: TagToken) { name = tag.name; attrs = tag.attrs }
    var source: String {
        Template.buildTag(name, attrs: attrs.keys.sorted().compactMap { key in
            attrs[key].map { Attribute(name: key, value: $0) }
        })
    }
    func string(_ key: String) -> String {
        if case .string(let value) = attrs[key] { return value }
        return ""
    }
    func bool(_ key: String) -> Bool { attrs[key] == .bool(true) }
    mutating func set(_ key: String, _ value: String) { attrs[key] = value.isEmpty ? nil : .string(value) }
    mutating func set(_ key: String, _ value: Bool) { attrs[key] = value ? .bool(true) : nil }
}
