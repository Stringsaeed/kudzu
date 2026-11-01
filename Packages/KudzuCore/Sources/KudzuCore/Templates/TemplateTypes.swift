import Foundation

/// A source-located parser or renderer observation.
public struct TemplateDiagnostic: Equatable, Sendable {
    /// Diagnostic importance for editor presentation.
    public enum Severity: String, Sendable { case warning, error }
    /// The diagnostic importance.
    public var severity: Severity
    /// A human-readable explanation.
    public var message: String
    /// Half-open UTF-16 offsets in the containing template source.
    public var range: Range<Int>
    /// Creates a source-located diagnostic.
    public init(severity: Severity, message: String, range: Range<Int>) {
        self.severity = severity; self.message = message; self.range = range
    }
}

/// A typed attribute value; quoted tokens always remain strings.
///
/// A bare token is a number only when it is an optionally signed plain decimal (`5`, `-3`, `+2.50`);
/// `true` and `false` are booleans; any other bare token is a string.
public enum AttributeValue: Equatable, Sendable {
    case string(String), bool(Bool), number(Double)
}

/// An ordered tag attribute.
public struct Attribute: Equatable, Sendable {
    /// Attribute identifier.
    public var name: String
    /// Typed attribute value.
    public var value: AttributeValue
    /// Creates an attribute.
    public init(name: String, value: AttributeValue) { self.name = name; self.value = value }
}

/// A recognized tag with ordered attributes and UTF-16 source offsets.
public struct Tag: Equatable, Sendable {
    /// Tag identifier.
    public var name: String
    /// Attributes in source order; duplicate names retain only their last occurrence.
    public var attributes: [Attribute]
    /// Half-open UTF-16 offsets, including delimiters.
    public var range: Range<Int>
    /// The exact source of an unknown or malformed `{{…}}` span, or nil for a recognized tag.
    ///
    /// Such a tag has no name or attributes. It renders and serializes as this text, so a tag a future version
    /// understands is not rewritten by an editor that does not.
    public var verbatim: String?
    /// Creates an editor tag, optionally without a source location.
    public init(name: String, attributes: [Attribute] = [], range: Range<Int> = 0..<0, verbatim: String? = nil) {
        self.name = name; self.attributes = attributes; self.range = range; self.verbatim = verbatim
    }
    func string(_ key: String) -> String? {
        guard case .string(let value) = attributes.last(where: { $0.name == key })?.value else { return nil }
        return value
    }
    func bool(_ key: String) -> Bool {
        guard case .bool(let value) = attributes.last(where: { $0.name == key })?.value else { return false }
        return value
    }
    /// The semantic view used by the editor and renderer.
    public var typed: TemplateTag { TemplateTag(self) }
}

/// A literal segment or recognized tag with source location.
public enum TemplateNode: Equatable, Sendable {
    case text(String, range: Range<Int>)
    case tag(Tag)
    /// Half-open UTF-16 offsets in the original source.
    public var range: Range<Int> {
        switch self { case .text(_, let range): range; case .tag(let tag): tag.range }
    }
}

/// A nonthrowing parse result.
public struct ParsedTemplate: Equatable, Sendable {
    /// Ordered literal and tag nodes.
    public var nodes: [TemplateNode]
    /// Recoverable syntax and validation issues.
    public var diagnostics: [TemplateDiagnostic]
}

/// Typed chip data derived from a generic tag.
public enum TemplateTag: Equatable, Sendable {
    case date(format: String, offset: DateOffset?, locale: String?)
    case time(format: String, offset: DateOffset?, locale: String?)
    case clipboard(trim: Bool)
    case cursor
    case snippet(id: UUID)
    case field(name: String, default: String?, multiline: Bool)
    case choice(name: String, options: [String], default: String?)
    case unknown

    /// Interprets a tag using v1 defaults.
    public init(_ tag: Tag) {
        guard tag.verbatim == nil else { self = .unknown; return }
        switch tag.name {
        case "date", "time":
            let format = tag.string("format") ?? (tag.name == "date" ? "yyyy-MM-dd" : "HH:mm")
            let offset = tag.string("offset").flatMap(DateOffset.init)
            self = tag.name == "date" ? .date(format: format, offset: offset, locale: tag.string("locale")) : .time(format: format, offset: offset, locale: tag.string("locale"))
        case "clipboard": self = .clipboard(trim: tag.bool("trim"))
        case "cursor": self = .cursor
        case "snippet":
            if let id = tag.string("id").flatMap(UUID.init(uuidString:)) { self = .snippet(id: id) } else { self = .unknown }
        case "field":
            if let name = tag.string("name") { self = .field(name: name, default: tag.string("default"), multiline: tag.bool("multiline")) } else { self = .unknown }
        case "choice":
            if let name = tag.string("name") { self = .choice(name: name, options: Self.options(tag.string("options") ?? ""), default: tag.string("default")) } else { self = .unknown }
        default: self = .unknown
        }
    }

    private static func options(_ source: String) -> [String] {
        guard !source.isEmpty else { return [] }
        let chars = Array(source)
        var result = [String](), item = "", i = 0
        while i < chars.count {
            if chars[i] == "\\", i + 1 < chars.count, chars[i + 1] == "|" {
                item.append("|"); i += 2
            } else if chars[i] == "|" {
                result.append(item); item = ""; i += 1
            } else { item.append(chars[i]); i += 1 }
        }
        result.append(item)
        return result
    }
}

/// A signed calendar offset with components applied in written order.
public struct DateOffset: Equatable, Sendable {
    private struct Part: Equatable, Sendable { var unit: String; var amount: Int }
    private var parts: [Part]

    /// Parses a signed offset such as +1w2d or -1y2m, returning nil for invalid input.
    public init?(_ source: String) {
        let chars = Array(source)
        guard let sign = chars.first, sign == "+" || sign == "-" else { return nil }
        var i = 1, parts = [Part]()
        while i < chars.count {
            let start = i
            while i < chars.count, chars[i].isASCII, chars[i].isNumber { i += 1 }
            guard i > start, let magnitude = Int(String(chars[start..<i])) else { return nil }
            let unitStart = i
            while i < chars.count, chars[i].isLetter { i += 1 }
            let unit = String(chars[unitStart..<i])
            guard ["d", "w", "m", "y", "h", "min"].contains(unit) else { return nil }
            parts.append(Part(unit: unit, amount: sign == "-" ? -magnitude : magnitude))
        }
        guard !parts.isEmpty else { return nil }
        self.parts = parts
    }

    /// Applies calendar arithmetic, preserving month-end clamping behavior.
    public func applying(to date: Date, calendar: Calendar) -> Date? {
        var result = date
        for part in parts {
            let component: Calendar.Component
            switch part.unit {
            case "d": component = .day
            case "w": component = .weekOfYear
            case "m": component = .month
            case "y": component = .year
            case "h": component = .hour
            default: component = .minute
            }
            guard let next = calendar.date(byAdding: component, value: part.amount, to: result) else { return nil }
            result = next
        }
        return result
    }
}
