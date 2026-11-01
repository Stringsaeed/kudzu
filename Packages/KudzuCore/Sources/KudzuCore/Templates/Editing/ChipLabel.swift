import Foundation

/// Display text, SF Symbol and accessibility description for one tag chip.
public struct ChipLabel: Equatable, Sendable {
    /// The chip family, which decides its tint.
    public enum Kind: String, Sendable, CaseIterable {
        case date, time, clipboard, cursor, snippet, field, choice, unknown
    }

    /// Chip family.
    public var kind: Kind
    /// SF Symbol name.
    public var symbolName: String
    /// Leading text, such as "Date" or a snippet name.
    public var title: String
    /// Optional secondary text shown after a middle dot.
    public var detail: String?
    /// Whether the chip should use a warning tint.
    public var isWarning: Bool
    /// VoiceOver description.
    public var accessibilityLabel: String

    /// The full visible text, e.g. "Date · yyyy-MM-dd".
    public var text: String { detail.map { "\(title) · \($0)" } ?? title }

    /// Describes `tag`; `snippetName` resolves referenced snippets (nil means missing).
    public init(tag: Tag, snippetName: (UUID) -> String? = { _ in nil }) {
        isWarning = false
        detail = nil
        switch tag.typed {
        case .date(let format, _, _), .time(let format, _, _):
            let isTime = tag.name == "time"
            kind = isTime ? .time : .date
            symbolName = isTime ? "clock" : "calendar"
            title = isTime ? "Time" : "Date"
            var parts = [DateFormatPreset.matching(format)?.chipText ?? format]
            if let offset = tag.stringAttribute("offset") {
                parts.append(offset)
                isWarning = DateOffset(offset) == nil
            }
            detail = parts.joined(separator: " ")
            var spoken = "\(title) tag, format \(format)"
            if let offset = tag.stringAttribute("offset") { spoken += ", offset \(offset)" }
            if let locale = tag.stringAttribute("locale") { spoken += ", locale \(locale)" }
            accessibilityLabel = spoken
        case .clipboard(let trim):
            kind = .clipboard
            symbolName = "doc.on.clipboard"
            title = "Clipboard"
            detail = trim ? "trimmed" : nil
            accessibilityLabel = trim ? "Clipboard tag, trimmed" : "Clipboard tag"
        case .cursor:
            kind = .cursor
            symbolName = "character.cursor.ibeam"
            title = "Cursor"
            accessibilityLabel = "Cursor position tag"
        case .snippet(let id):
            kind = .snippet
            symbolName = "text.badge.plus"
            if let name = snippetName(id) {
                title = name.isEmpty ? "Untitled snippet" : name
                accessibilityLabel = "Snippet tag, \(title)"
            } else {
                title = "Missing snippet"
                isWarning = true
                accessibilityLabel = "Snippet tag, missing snippet"
            }
        case .field(let name, _, let multiline):
            kind = .field
            symbolName = "character.textbox"
            title = "Field"
            detail = name.isEmpty ? "unnamed" : name
            isWarning = name.isEmpty
            accessibilityLabel = "\(multiline ? "Multiline text field" : "Text field") tag, \(detail ?? "")"
        case .choice(let name, let options, _):
            kind = .choice
            symbolName = "list.bullet"
            title = "Choice"
            detail = name.isEmpty ? "unnamed" : name
            isWarning = name.isEmpty || options.isEmpty
            accessibilityLabel = "Dropdown tag, \(detail ?? ""), \(options.count) option\(options.count == 1 ? "" : "s")"
        case .unknown:
            kind = .unknown
            symbolName = "questionmark.diamond"
            // An unrecognized span is shown exactly as written, since that is how it will be saved.
            let source = tag.verbatim ?? tag.source
            title = source
            isWarning = true
            accessibilityLabel = "Unrecognized tag \(source), kept as written"
        }
    }
}
