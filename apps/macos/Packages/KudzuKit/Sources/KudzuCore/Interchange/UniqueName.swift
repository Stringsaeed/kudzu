// Ported from kudzu-core/src/interchange/unique_name.rs
// Names and triggers that don't collide with existing ones.

import Foundation

/// Names and triggers that don't collide with existing ones.
public enum UniqueName {
    /// `base`, or `base 2`, `base 3`… the first not already in `existing` (case-insensitive).
    public static func make(_ base: String, existing: [String]) -> String { free(base,existing," ") }
    /// Like [`make`], but numbers without a space (`;snippet`, `;snippet2`), which suits triggers.
    public static func makeTrigger(_ base: String, existing: [String]) -> String { free(base,existing,"") }
    static func free(_ base: String, _ existing: [String], _ separator: String) -> String {
        let taken = Set(existing.map { Data($0.lowercased().utf8) })
        if !taken.contains(Data(base.lowercased().utf8)) { return base }
        var n = 2
        while taken.contains(Data("\(base)\(separator)\(n)".lowercased().utf8)) { n += 1 }
        return "\(base)\(separator)\(n)"
    }
    /// First nonblank line, truncated at a grapheme boundary, or fallback for blank content.
    public static func nameFromContent(_ content: String, fallback: String, limit: Int) -> String {
        // `split` on the explicit line separators, then `trim` (Unicode White_Space, not U+200B).
        let separators: Set<Unicode.Scalar> = ["\n","\r","\u{0b}","\u{0c}","\u{85}","\u{2028}","\u{2029}"]
        var lines: [String] = [], current: [Unicode.Scalar] = []
        for scalar in content.unicodeScalars {
            if separators.contains(scalar) { lines.append(String(String.UnicodeScalarView(current))); current = [] }
            else { current.append(scalar) }
        }
        lines.append(String(String.UnicodeScalarView(current)))
        let line = lines.map(RustText.trim).first { !$0.isEmpty } ?? ""
        if line.isEmpty { return fallback }
        // `graphemes(true)` then `trim_end` only, so a mark at the cut is not stripped from the front.
        if line.count <= limit { return line }
        return RustText.trimEnd(String(line.prefix(max(0,limit)))) + "…"
    }
}
