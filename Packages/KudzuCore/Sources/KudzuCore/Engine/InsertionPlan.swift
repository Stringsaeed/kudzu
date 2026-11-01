/// Whether the triggering event can be withheld from the destination.
public enum ExpansionTapMode: Sendable {
    case active, listenOnly
}

/// Platform-independent edits needed to replace a matched trigger.
public struct InsertionPlan: Equatable, Sendable {
    /// Backspaces before inserting text.
    public let deleteCount: Int
    /// Rendered, case-propagated expansion.
    public let text: String
    /// Whole-word terminator to type after the expansion.
    public let reemitSeparator: String?
    /// Left-arrow presses after text and separator have been inserted.
    public let caretLeftCount: Int

    /// Plans edits; cursor offsets are UTF-16, while arrow counts are grapheme clusters.
    public init(match: TriggerMatch, text: String, cursorOffset: Int?, mode: ExpansionTapMode) {
        self.text = text
        reemitSeparator = match.separator
        switch mode {
        case .active:
            deleteCount = max(0, match.deleteCount - (match.fireMode == .immediate ? 1 : 0))
        case .listenOnly:
            deleteCount = match.deleteCountIncludingSeparator
        }
        if let cursorOffset {
            let offset = min(max(0, cursorOffset), text.utf16.count)
            // Snap an offset inside a grapheme to its start; never split emoji or CRLF.
            var position = 0
            var after = 0
            for character in text {
                if position >= offset || position + String(character).utf16.count > offset { after += 1 }
                position += String(character).utf16.count
            }
            caretLeftCount = after + (match.separator?.count ?? 0)
        } else {
            caretLeftCount = 0
        }
    }
}
