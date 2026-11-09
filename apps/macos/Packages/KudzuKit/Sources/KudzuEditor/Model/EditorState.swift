// The editor's model state and its pure transitions: the port of `formattingField`, `setFormatting`,
// `changeFormat`, `toggleList` and the image insertion in `image-tools.tsx`. The NSTextView layer
// only renders a state and turns user input into `FormattingEdit`s; everything here is testable
// without AppKit. Offsets are UTF-16, like the TS and the stored JSON.

import Foundation
import KudzuCore

/// Source text, formatting (`nil` = plain text) and selection, all in source coordinates.
public struct EditorState: Sendable, CustomStringConvertible {
    public var source: String
    /// `nil` in plain mode; `[]` or more in formatted mode.
    public var spans: [FormatSpan]?
    public var selection: Range<Int>

    public init(source: String, spans: [FormatSpan]? = nil, selection: Range<Int>? = nil) {
        self.source = source
        self.spans = spans.map { FormattingHelpers.validSpans(source, $0, isValidLink: EditorLinkRules.safeLink) }
        let end = source.utf16.count
        self.selection = selection.map { min($0.lowerBound, end)..<min($0.upperBound, end) } ?? end..<end
    }

    /// Loads the bound value the way the TS editor does: spans only when `formatting.source` matches.
    public init(source: String, formatting: Formatting?) {
        let spans = formatting.flatMap { $0.source.utf16.elementsEqual(source.utf16) ? $0.spans : nil }
        self.init(source: source, spans: spans)
    }

    public var isFormatted: Bool { spans != nil }

    /// The `Formatting` to store beside `source`, or `nil` in plain mode.
    public var formatting: Formatting? { spans.map { Formatting(source: source, spans: $0) } }

    /// The selection widened across chips and images it touches (`selectedRange`).
    public var selectedRange: Range<Int> {
        EditorFormatting.selectedRange(selection, source: source, spans: spans)
    }

    /// Byte-for-byte equal source (no Unicode normalization), same spans and selection.
    public func same(as other: EditorState) -> Bool {
        source.utf16.elementsEqual(other.source.utf16) && FormattingHelpers.sameSpans(spans, other.spans) && selection == other.selection
    }

    public var description: String { "EditorState(\(source.debugDescription), spans: \(spans.map { "\($0)" } ?? "nil"), selection: \(selection))" }

    // MARK: Edits

    /// Applies disjoint edits (ascending, old coordinates): text marks grow at their edges, images
    /// keep inserted text outside, and spans that no longer fit are dropped (`validSpans`).
    public func applying(_ edits: [FormattingEdit], selection: Range<Int>? = nil) -> EditorState {
        let text = NSMutableString(string: source)
        for edit in edits.sorted(by: { $0.range.lowerBound > $1.range.lowerBound }) {
            text.replaceCharacters(in: NSRange(location: edit.range.lowerBound, length: edit.range.count), with: edit.insert)
        }
        let next = text as String
        let spans = EditorFormatting.mapSpans(spans, changes: edits.sorted { $0.range.lowerBound < $1.range.lowerBound })
        let mapped = selection ?? Self.map(self.selection, through: edits)
        return EditorState(source: next, spans: spans, selection: mapped)
    }

    public func applying(_ edit: FormattingEdit, selection: Range<Int>? = nil) -> EditorState {
        applying([edit], selection: selection)
    }

    /// Replaces `range` with `text` and puts the caret after it (`replace` in the handle).
    public func replacing(_ range: Range<Int>, with text: String) -> EditorState {
        let end = range.lowerBound + text.utf16.count
        return applying(FormattingEdit(range: range, insert: text), selection: end..<end)
    }

    /// CodeMirror's `SelectionRange.map`: a caret stays before text inserted at it; a range keeps
    /// insertions at its edges outside.
    static func map(_ selection: Range<Int>, through edits: [FormattingEdit]) -> Range<Int> {
        let sorted = edits.sorted { $0.range.lowerBound < $1.range.lowerBound }
        if selection.isEmpty {
            let at = mapPosition(selection.lowerBound, through: sorted, assoc: -1)
            return at..<at
        }
        let from = mapPosition(selection.lowerBound, through: sorted, assoc: 1)
        let to = mapPosition(selection.upperBound, through: sorted, assoc: -1)
        return min(from, to)..<max(from, to)
    }

    /// The same position mapping `EditorFormatting.mapSpans` uses for span edges.
    static func mapPosition(_ position: Int, through edits: [FormattingEdit], assoc: Int) -> Int {
        var delta = 0
        for edit in edits {
            let from = edit.range.lowerBound, to = edit.range.upperBound, length = edit.insert.utf16.count
            if position < from { break }
            if position > to { delta += length - (to - from); continue }
            if position == from, from != to { return from + delta }
            if position == to, from != to { return from + delta + length }
            return from + delta + (assoc < 0 ? 0 : length)
        }
        return position + delta
    }

    // MARK: Formatting

    /// `setFormatting.of(formatted ? [] : null)`: switching to plain drops every span, so images
    /// fall back to their text alternatives; switching back starts with no spans.
    public func settingFormatted(_ formatted: Bool) -> EditorState {
        guard formatted != isFormatted else { return self }
        var next = self
        next.spans = formatted ? [] : nil
        return next
    }

    /// `changeFormat`: toggles a style over the selection, replaces a link of the same kind, or
    /// (`nil`) clears every style in it. Returns `nil` when nothing applies, like the TS `false`.
    public func changingFormat(_ style: KudzuCore.FormatStyle?) -> EditorState? {
        let range = selectedRange
        guard !range.isEmpty, let spans else { return nil }
        var next = self
        next.spans = FormattingHelpers.validSpans(source, EditorFormatting.changeFormat(spans, range: range, style: style), isValidLink: EditorLinkRules.safeLink)
        return next
    }

    /// Whether a span of `kind` covers the whole selection (the toolbar's pressed state).
    public func isActive(_ kind: String) -> Bool {
        let range = selectedRange
        return spans?.contains { $0.style.kind == kind && $0.start <= range.lowerBound && $0.end >= range.upperBound } ?? false
    }

    /// The link covering the selection, for the link popover.
    public var linkAtSelection: String? {
        let range = selectedRange
        for span in spans ?? [] where span.start <= range.lowerBound && span.end >= range.upperBound {
            if case .link(let url) = span.style { return url }
        }
        return nil
    }

    /// `toggleList`: adds or removes `• ` / `1. ` prefixes on the selected lines (real text).
    public func togglingList(ordered: Bool) -> EditorState {
        let edits = EditorFormatting.toggleList(source, selection: selection, ordered: ordered)
        return applying(edits)
    }

    // MARK: Images

    public enum ImageError: Error, Equatable, LocalizedError {
        /// More than 512 KiB of image data URLs in one snippet (384 KiB of PNG).
        case snippetTooLarge

        public var errorDescription: String? {
            switch self {
            case .snippetTooLarge: "This snippet has too many images. Remove an image or use a smaller one."
            }
        }
    }

    /// The most image data (data-URL characters) one snippet may hold.
    public static let snippetImageLimit = 524_288

    /// Inserts `[label]` over the selection as an image span (`image-tools.tsx` submit). The label
    /// loses braces and line breaks, and defaults to "Image".
    public func insertingImage(dataUrl: String, width: Int, label: String) throws(ImageError) -> EditorState {
        let range = selectedRange
        var clean = label.trimmingCharacters(in: .whitespacesAndNewlines)
        // JS replaces individual code units: CRLF becomes two spaces, and a brace followed by
        // a combining mark is still removed. Swift Character iteration would group both cases.
        clean = String(String.UnicodeScalarView(clean.unicodeScalars.map { "{}\r\n".unicodeScalars.contains($0) ? " " : $0 }))
        let text = "[\(clean.isEmpty ? "Image" : clean)]"
        let end = range.lowerBound + text.utf16.count
        var next = applying(FormattingEdit(range: range, insert: text), selection: end..<end)
        if next.spans == nil { next.spans = [] }
        var spans = (next.spans ?? []).filter { span in
            guard case .image = span.style else { return true }
            return !(span.start < end && span.end > range.lowerBound)
        }
        let size = spans.reduce(dataUrl.utf16.count) { sum, span in
            if case .image(let url, _) = span.style { return sum + url.utf16.count }
            return sum
        }
        guard size <= Self.snippetImageLimit else { throw .snippetTooLarge }
        spans.append(FormatSpan(start: range.lowerBound, end: end, style: .image(dataUrl: dataUrl, width: UInt32(min(512, max(1, width))))))
        next.spans = FormattingHelpers.validSpans(next.source, spans, isValidLink: EditorLinkRules.safeLink)
        return next
    }
}
