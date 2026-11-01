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
    ///
    /// `swallowedText` is the text of the keystroke the active tap withheld (the final trigger
    /// key for immediate triggers). It matters because one keystroke doesn't always add one
    /// grapheme: a combining mark merges into the previous character.
    public init(match: TriggerMatch, text: String, cursorOffset: Int?, mode: ExpansionTapMode,
                swallowedText: String? = nil) {
        self.text = text
        reemitSeparator = match.separator
        switch mode {
        case .active where match.fireMode == .immediate:
            let typed = match.typedTrigger.unicodeScalars
            if let swallowed = swallowedText?.unicodeScalars, !swallowed.isEmpty,
               typed.count >= swallowed.count, typed.suffix(swallowed.count).elementsEqual(swallowed) {
                // What the document actually shows: the trigger minus the withheld keystroke.
                var visible = String.UnicodeScalarView()
                visible.append(contentsOf: typed.dropLast(swallowed.count))
                deleteCount = String(visible).count
            } else {
                deleteCount = max(0, match.deleteCount - 1)
            }
        case .active:
            deleteCount = match.deleteCount
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
