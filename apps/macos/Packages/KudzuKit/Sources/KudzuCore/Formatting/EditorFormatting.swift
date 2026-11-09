import Foundation

/// An edit in the old document's UTF-16 coordinates. Supply disjoint edits in ascending order.
public struct FormattingEdit: Hashable, Sendable {
    public var range: Range<Int>
    public var insert: String
    public init(range: Range<Int>, insert: String) { self.range = range; self.insert = insert }
}
public enum EditorFormatting {
    /// Text marks grow at their edges; image ranges associate inward, keeping inserted text outside.
    public static func mapSpans(_ spans: [FormatSpan]?, changes: [FormattingEdit]) -> [FormatSpan]? {
        func map(_ position: Int, assoc: Int) -> Int {
            var delta = 0
            for edit in changes {
                let from = edit.range.lowerBound, to = edit.range.upperBound, length = edit.insert.utf16.count
                if position < from { break }
                if position > to { delta += length - (to - from); continue }
                if position == from, from != to { return from + delta }
                if position == to, from != to { return from + delta + length }
                return from + delta + (assoc < 0 ? 0 : length)
            }
            return position + delta
        }
        return spans?.compactMap { span in
            let image = span.style.kind == "image"
            let start = map(span.start, assoc: image ? 1 : -1), end = map(span.end, assoc: image ? -1 : 1)
            return start < end ? FormatSpan(start: start, end: end, style: span.style) : nil
        }
    }
    /// Expands nonempty selection across intersecting variable chips and images.
    public static func selectedRange(_ range: Range<Int>, source: String, spans: [FormatSpan]?) -> Range<Int> {
        if range.isEmpty { return range }
        var from = range.lowerBound, to = range.upperBound
        for token in Template.tokenize(source) {
            if case .tag(let tag) = token, tag.from < to, tag.to > from { from = min(from, tag.from); to = max(to, tag.to) }
        }
        for s in spans ?? [] where s.style.kind == "image" && s.start < to && s.end > from { from = min(from, s.start); to = max(to, s.end) }
        return from..<to
    }
    /// Toggle a style, replace a link/image of the same kind, or clear selected formatting.
    public static func changeFormat(_ spans: [FormatSpan], range: Range<Int>, style: FormatStyle?) -> [FormatSpan] {
        if range.isEmpty { return spans }
        let from = range.lowerBound, to = range.upperBound
        let remove = style == nil || spans.contains { span in span.start <= from && span.end >= to && style.map { FormattingHelpers.sameStyle(span.style,$0) } == true }
        var next = spans.flatMap { s -> [FormatSpan] in
            if s.end <= from || s.start >= to || (style != nil && s.style.kind != style?.kind) { return [s] }
            var pieces: [FormatSpan] = []
            if s.start < from { pieces.append(FormatSpan(start: s.start, end: from, style: s.style)) }
            if s.end > to { pieces.append(FormatSpan(start: to, end: s.end, style: s.style)) }
            return pieces
        }
        if !remove, let style { next.append(FormatSpan(start: from, end: to, style: style)) }
        return next
    }
    /// Prefixes remain real text so lists keep their numbering in plain-text apps and exports.
    public static func toggleList(_ source: String, selection: Range<Int>, ordered: Bool) -> [FormattingEdit] {
        let lines = source.components(separatedBy: "\n"); var at = 0, selected: [(Int, String)] = []
        let end = selection.isEmpty ? selection.upperBound : selection.upperBound - 1
        for line in lines {
            let next = at + line.utf16.count
            if selection.lowerBound <= next && end >= at { selected.append((at, line)) }
            at = next + 1
        }
        // JavaScript `\d` is ASCII only.
        let pattern = ordered ? #"^[0-9]+\. "# : "^• "
        let remove = selected.allSatisfy { $0.1.range(of: pattern, options: .regularExpression) != nil }
        return selected.enumerated().map { i, entry in
            let marker = entry.1.range(of: #"^(?:• |[0-9]+\. )"#, options: .regularExpression).map { entry.1[$0].utf16.count } ?? 0
            return FormattingEdit(range: entry.0..<(entry.0 + marker), insert: remove ? "" : ordered ? "\(i + 1). " : "• ")
        }
    }
}
