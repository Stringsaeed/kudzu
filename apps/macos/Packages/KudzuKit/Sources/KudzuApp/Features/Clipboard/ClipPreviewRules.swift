import Foundation
import KudzuCore

/// Preview truncation and highlight ranges from `clip-content.tsx` and `Marked` in `clip-list.tsx`.
/// Text is cut with JavaScript `String.prototype.slice`: UTF-16 code units. A cut surrogate pair is
/// dropped, because a Swift string cannot hold half of one; the hidden count includes that unit.
enum ClipPreviewRules {
    /// Characters rendered before "…and N more" (`RENDER_LIMIT`).
    static let renderLimit = 60_000
    /// Code lines rendered before "…and N more lines" (`CODE_LINE_LIMIT`).
    static let codeLineLimit = 1_500
    /// List row title/subtitle cap (`title.slice(0, 200)` in `clip-list.tsx`).
    static let rowTextLimit = 200

    struct Slice: Equatable, Sendable {
        var shown: String
        var hidden: Int
    }

    struct TextPreview: Equatable, Sendable {
        var shown: String
        var hiddenCharacters: Int
        /// UTF-16 ranges of `terms` inside `shown`. A match past the cut is absent.
        var ranges: [Range<Int>]
    }

    struct CodePreview: Equatable, Sendable {
        var lines: [String]
        var hiddenLines: Int
    }

    static func prefix(_ text: String, _ limit: Int) -> Slice {
        let units = Array(text.utf16)
        guard limit >= 0, units.count > limit else { return Slice(shown: text, hidden: 0) }
        var end = limit
        if end > 0, UTF16.isLeadSurrogate(units[end - 1]) { end -= 1 }
        return Slice(shown: String(decoding: units.prefix(end), as: UTF16.self), hidden: units.count - end)
    }

    static func textPreview(_ text: String, terms: [String]) -> TextPreview {
        let slice = prefix(text, renderLimit)
        return TextPreview(shown: slice.shown, hiddenCharacters: slice.hidden, ranges: Clipboard.matchRanges(slice.shown, terms: terms))
    }

    /// `split("\n")`, then the first `CODE_LINE_LIMIT` lines.
    static func codePreview(_ text: String) -> CodePreview {
        let lines = text.components(separatedBy: "\n")
        guard lines.count > codeLineLimit else { return CodePreview(lines: lines, hiddenLines: 0) }
        return CodePreview(lines: Array(lines.prefix(codeLineLimit)), hiddenLines: lines.count - codeLineLimit)
    }

    static func moreCharacters(_ hidden: Int) -> String? {
        guard hidden > 0 else { return nil }
        return "…and \(hidden.formatted()) more characters. Copy it to get everything."
    }

    static func moreLines(_ hidden: Int) -> String? {
        guard hidden > 0 else { return nil }
        return "…and \(hidden.formatted()) more lines."
    }

    /// Visible list text before search highlighting (`Marked` in `clip-list.tsx`).
    static func rowText(_ text: String) -> String {
        prefix(text, rowTextLimit).shown
    }

    /// `loadFullText` in `lib/clipboard.ts`: excerpt is complete when `textLen <= text.length` (UTF-16).
    static func excerptIsComplete(textLen: Int, excerpt: String?) -> Bool {
        guard let excerpt else { return false }
        return textLen <= excerpt.utf16.count
    }

    /// `buildRows` `pinnedFirst` in `clipboard-view.tsx` / `clipboard-picker.tsx`.
    static func pinnedFirst(picker: Bool, query: String, filter: String) -> Bool {
        guard !picker else { return false }
        // `!query` in clipboard-view.tsx. A space is still a query.
        return query.isEmpty && filter != "pinned"
    }

    /// `CopyTimeline` window start (`times[0] ?? copiedAt` in `clip-preview.tsx`).
    static func copyTimelineAnchor(timings: [Date], copiedAt: Date) -> Date {
        timings.first ?? copiedAt
    }
}
