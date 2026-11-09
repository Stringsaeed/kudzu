import Foundation
import KudzuCore

// Platform-independent edit planning: how many backspaces, what to type, where the caret goes.
// Port of `kudzu-engine/src/plan.rs`.

/// Whether the keystroke that completed a trigger could be withheld from the target app.
enum PlanMode: Hashable, Sendable {
    /// The tap swallowed the final key (immediate) or the separator (whole word).
    case active
    /// Every typed key, including any separator, already reached the document.
    case listenOnly
}

/// Edits needed to replace a matched trigger.
struct InsertionPlan: Hashable, Sendable {
    /// Backspaces before inserting text.
    var deleteCount: Int
    /// Rendered, case-propagated expansion.
    var text: String
    /// Whole-word terminator to type after the expansion.
    var reemitSeparator: String?
    /// Left-arrow presses after text and separator are inserted.
    var caretLeftCount: Int

    /// Plans edits. `cursorOffset` is in UTF-16 units of `text`; arrow counts are grapheme clusters.
    ///
    /// `swallowed` is the text of the keystroke an active tap withheld. One keystroke doesn't always
    /// add one grapheme (a combining mark merges into the previous character), so the visible
    /// remainder of the trigger is measured rather than assumed.
    init(_ m: TriggerMatch, text: String, cursorOffset: Int?, mode: PlanMode, swallowed: String?) {
        switch mode {
        case .active where m.fireMode == .immediate:
            if let swallowed, !swallowed.isEmpty, let visible = Plan.stripScalarSuffix(m.typedTrigger, swallowed) {
                deleteCount = Plan.graphemeCount(visible)
            } else {
                deleteCount = max(0, m.deleteCount - 1)
            }
        case .active:
            deleteCount = m.deleteCount
        case .listenOnly:
            deleteCount = m.deleteCountIncludingSeparator
        }
        if let cursorOffset {
            caretLeftCount = Plan.graphemesAfterUTF16(text, cursorOffset) + (m.separator.map(Plan.graphemeCount) ?? 0)
        } else {
            caretLeftCount = 0
        }
        self.text = text
        reemitSeparator = m.separator
    }
}

/// Grapheme and UTF-16 helpers shared by planning and insertion.
enum Plan {
    /// Number of grapheme clusters (what one backspace or arrow press moves over).
    static func graphemeCount(_ text: String) -> Int {
        text.count
    }

    /// Graphemes at or after UTF-16 `offset`; an offset inside a grapheme snaps to its start.
    static func graphemesAfterUTF16(_ text: String, _ offset: Int) -> Int {
        var position = 0
        var after = 0
        for g in text {
            let length = g.utf16.count
            if position >= offset || position + length > offset {
                after += 1
            }
            position += length
        }
        return after
    }

    /// The prefix of `text` covering at most `offset` UTF-16 units (never splits a scalar).
    static func utf16Prefix(_ text: String, _ offset: Int) -> String {
        var units = 0
        var scalars = String.UnicodeScalarView()
        for scalar in text.unicodeScalars {
            let length = UTF16.width(scalar)
            if units + length > offset {
                break
            }
            units += length
            scalars.append(scalar)
        }
        return String(scalars)
    }

    /// Re-maps a UTF-16 cursor offset after case propagation (which can change lengths, e.g. ß → SS).
    static func casedCursorOffset(_ style: CaseStyle, _ rendered: String, _ offset: Int) -> Int {
        if style == .asIs {
            return offset
        }
        return CasePropagation.apply(style, to: utf16Prefix(rendered, offset)).utf16.count
    }

    static func casedFormatting(_ style: CaseStyle, _ text: String, _ spans: [FormatSpan]) -> [FormatSpan] {
        spans.map { span in
            FormatSpan(
                start: casedCursorOffset(style, text, span.start),
                end: casedCursorOffset(style, text, span.end),
                style: span.style
            )
        }
    }

    /// Choose the attachment or plain-text shape only when the app's caret confirms it.
    /// Unknown delivery must not arm a destructive Backspace undo or move the caret by a guess.
    static func imageDelivery(_ text: String, _ spans: [FormatSpan], cursor: Int?, observed: Int?) -> (text: String, cursor: Int?)? {
        guard let length = observed else { return nil }
        if length == text.utf16.count {
            return (text, cursor)
        }
        let visual = FormattingHelpers.insertionText(text, spans: spans)
        guard length == visual.utf16.count else { return nil }
        return (visual, cursor.map { FormattingHelpers.insertionOffset($0, spans: spans) })
    }

    /// Inserted graphemes minus typed trigger graphemes, never negative.
    static func charsSaved(_ inserted: String, _ typedTrigger: String) -> Int {
        max(0, graphemeCount(inserted) - graphemeCount(typedTrigger))
    }

    /// Whether `method` should deliver `text` as keystrokes (true) or via paste (false).
    ///
    /// Automatic types short text with at most one line break; long or multi-line text pastes, because
    /// synthetic keystrokes are slow and auto-indenting editors mangle typed newlines.
    static func shouldType(_ method: InsertionMethod, _ text: String) -> Bool {
        switch method {
        case .type:
            return true
        case .paste:
            return false
        case .automatic:
            var graphemes = 0
            var newlines = 0
            for g in text {
                graphemes += 1
                if let first = g.unicodeScalars.first, isNewline(first) {
                    newlines += 1
                }
            }
            return graphemes < 100 && newlines < 2
        }
    }

    private static func isNewline(_ c: Unicode.Scalar) -> Bool {
        switch c.value {
        case 0x0A, 0x0D, 0x0B, 0x0C, 0x85, 0x2028, 0x2029: true
        default: false
        }
    }

    /// `text` without the scalar suffix `suffix` (Rust `str::strip_suffix`, which compares code
    /// points rather than Swift's canonically equivalent characters), or nil when it doesn't end so.
    static func stripScalarSuffix(_ text: String, _ suffix: String) -> String? {
        let scalars = Array(text.unicodeScalars)
        let tail = Array(suffix.unicodeScalars)
        guard tail.count <= scalars.count, Array(scalars.suffix(tail.count)) == tail else { return nil }
        var view = String.UnicodeScalarView()
        view.append(contentsOf: scalars.dropLast(tail.count))
        return String(view)
    }
}
