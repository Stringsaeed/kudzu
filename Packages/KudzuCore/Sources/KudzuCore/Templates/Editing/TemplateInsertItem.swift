import Foundation

/// An entry in the editor's `/` and `+` insert menus.
public enum TemplateInsertItem: String, CaseIterable, Sendable, Identifiable {
    case date, time, clipboard, cursor, field, choice, snippet

    /// Stable identifier.
    public var id: String { rawValue }

    /// Menu title.
    public var title: String {
        switch self {
        case .date: "Date"
        case .time: "Time"
        case .clipboard: "Clipboard"
        case .cursor: "Cursor"
        case .field: "Text Field"
        case .choice: "Dropdown"
        case .snippet: "Snippet…"
        }
    }

    /// One-line explanation shown beside the title.
    public var subtitle: String {
        switch self {
        case .date: "Today's date, optionally offset"
        case .time: "The current time"
        case .clipboard: "Whatever is on the clipboard"
        case .cursor: "Where the caret lands"
        case .field: "Ask for text when expanding"
        case .choice: "Pick from a list when expanding"
        case .snippet: "Include another snippet"
        }
    }

    /// SF Symbol name, matching the chip.
    public var symbolName: String {
        switch self {
        case .date: "calendar"
        case .time: "clock"
        case .clipboard: "doc.on.clipboard"
        case .cursor: "character.cursor.ibeam"
        case .field: "character.textbox"
        case .choice: "list.bullet"
        case .snippet: "text.badge.plus"
        }
    }

    /// Extra words matched by the `/` filter.
    public var keywords: [String] {
        switch self {
        case .date: ["day", "today", "calendar"]
        case .time: ["hour", "clock", "now"]
        case .clipboard: ["paste", "copy"]
        case .cursor: ["caret", "position"]
        case .field: ["input", "text", "form"]
        case .choice: ["select", "list", "options", "menu", "choice", "form"]
        case .snippet: ["include", "nested", "embed"]
        }
    }

    /// Whether inserting this item should immediately open its settings popover.
    public var editsOnInsert: Bool { [.field, .choice, .snippet].contains(self) }

    /// Items whose title or keywords match `query`, best matches first.
    public static func matching(_ query: String) -> [TemplateInsertItem] {
        let needle = query.lowercased()
        guard !needle.isEmpty else { return allCases }
        func rank(_ item: TemplateInsertItem) -> Int? {
            let words = item.title.lowercased().split { !$0.isLetter }.map(String.init)
            if item.title.lowercased().hasPrefix(needle) { return 0 }
            if words.contains(where: { $0.hasPrefix(needle) }) { return 1 }
            if item.keywords.contains(where: { $0.hasPrefix(needle) }) || item.rawValue.hasPrefix(needle) { return 2 }
            return nil
        }
        return allCases.compactMap { item in rank(item).map { (item, $0) } }
            .sorted { $0.1 < $1.1 }
            .map(\.0)
    }

    /// The tag inserted for this item; form names are made unique against `existing` content.
    public func makeTag(existing: [TemplateSegment] = [], snippetID: UUID? = nil) -> Tag {
        switch self {
        case .date: Tag(name: "date")
        case .time: Tag(name: "time")
        case .clipboard: Tag(name: "clipboard")
        case .cursor: Tag(name: "cursor")
        case .field: Tag(name: "field", attributes: [Attribute(name: "name", value: .string(Self.uniqueName("field", existing)))])
        case .choice: Tag(name: "choice", attributes: [Attribute(name: "name", value: .string(Self.uniqueName("choice", existing)))])
        case .snippet:
            // A snippet tag needs a UUID to parse; the nil UUID reads as "Missing snippet" until one is picked.
            Tag(name: "snippet", attributes: [Attribute(name: "id", value: .string((snippetID ?? Self.placeholderSnippetID).uuidString))])
        }
    }

    /// The reference used by a freshly inserted snippet chip before the user picks a target.
    public static let placeholderSnippetID = UUID(uuid: (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0))

    private static func uniqueName(_ base: String, _ segments: [TemplateSegment]) -> String {
        var used = Set<String>()
        for case .chip(let tag) in segments {
            switch tag.typed {
            case .field(let name, _, _), .choice(let name, _, _): used.insert(name)
            default: break
            }
        }
        var candidate = base, counter = 2
        while used.contains(candidate) { candidate = "\(base)\(counter)"; counter += 1 }
        return candidate
    }
}

/// Detects the `/query` the user is typing to filter the insert menu.
public enum SlashCommand {
    /// The longest query kept open; anything longer is treated as ordinary text.
    public static let maxQueryLength = 24

    /// Whether a `/` typed at `offset` may open the menu: at line start or after whitespace.
    public static func canOpen(in text: String, slashAt offset: Int) -> Bool {
        let units = Array(text.utf16)
        guard offset >= 0, offset < units.count, units[offset] == UInt16(UInt8(ascii: "/")) else { return false }
        guard offset > 0 else { return true }
        guard let scalar = Unicode.Scalar(units[offset - 1]) else { return false }
        return scalar.properties.isWhitespace
    }

    /// The query between the slash at `slashOffset` and `caret`, or nil if the session should close.
    public static func query(in text: String, slashAt slashOffset: Int, caret: Int) -> String? {
        let units = Array(text.utf16)
        guard slashOffset >= 0, slashOffset < units.count, units[slashOffset] == UInt16(UInt8(ascii: "/")),
              caret > slashOffset, caret <= units.count,
              caret - slashOffset - 1 <= maxQueryLength else { return nil }
        let query = String(decoding: units[(slashOffset + 1)..<caret], as: UTF16.self)
        guard query.unicodeScalars.allSatisfy({ $0.properties.isAlphabetic || ("0"..."9").contains($0) }) else { return nil }
        return query
    }
}
