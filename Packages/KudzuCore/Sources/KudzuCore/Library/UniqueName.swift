import Foundation

/// Generates names and triggers that don't collide with existing ones.
public enum UniqueName {
    /// `base`, or `base 2`, `base 3`… the first not already in `existing` (case-insensitive).
    public static func make(base: String, existing: some Sequence<String>) -> String {
        let taken = Set(existing.map { $0.lowercased() })
        if !taken.contains(base.lowercased()) { return base }
        var n = 2
        while taken.contains("\(base) \(n)".lowercased()) { n += 1 }
        return "\(base) \(n)"
    }

    /// Like `make`, but numbers without a space (`;snippet`, `;snippet2`), which suits triggers.
    public static func makeTrigger(base: String, existing: some Sequence<String>) -> String {
        let taken = Set(existing.map { $0.lowercased() })
        if !taken.contains(base.lowercased()) { return base }
        var n = 2
        while taken.contains("\(base)\(n)".lowercased()) { n += 1 }
        return "\(base)\(n)"
    }

    /// A short snippet name derived from content: its first non-empty line, truncated.
    /// Falls back to `fallback` when the content is blank.
    public static func name(fromContent content: String, fallback: String, limit: Int = 40) -> String {
        let line = content.split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .first { !$0.isEmpty } ?? ""
        guard !line.isEmpty else { return fallback }
        return line.count <= limit ? line : String(line.prefix(limit)).trimmingCharacters(in: .whitespaces) + "…"
    }
}
