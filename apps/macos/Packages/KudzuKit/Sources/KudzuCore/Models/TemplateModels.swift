import Foundation

// Serializable types of `kudzu-core/src/template/mod.rs`. The parser, renderer and analysis live
// in `Template/`.

/// How serious a template diagnostic is.
public enum Severity: String, Codable, Hashable, Sendable {
    case warning
    case error
}

/// A problem in a template, located by UTF-16 offsets into the source.
public struct Diagnostic: Codable, Hashable, Sendable {
    public var severity: Severity
    public var message: String
    /// Half-open UTF-16 range; JSON `{"start", "end"}` like serde's `Range<usize>`.
    public var range: Range<Int>

    public init(severity: Severity, message: String, range: Range<Int>) {
        self.severity = severity
        self.message = message
        self.range = range
    }

    private enum CodingKeys: String, CodingKey { case severity, message, range }
    private enum RangeKeys: String, CodingKey { case start, end }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        severity = try c.decode(Severity.self, forKey: .severity)
        message = try c.decode(String.self, forKey: .message)
        let r = try c.nestedContainer(keyedBy: RangeKeys.self, forKey: .range)
        let start = try r.decode(Int.self, forKey: .start)
        let end = try r.decode(Int.self, forKey: .end)
        range = start..<max(start, end)
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(severity, forKey: .severity)
        try c.encode(message, forKey: .message)
        var r = c.nestedContainer(keyedBy: RangeKeys.self, forKey: .range)
        try r.encode(range.lowerBound, forKey: .start)
        try r.encode(range.upperBound, forKey: .end)
    }
}

/// The control a form field uses.
public enum FormFieldKind: String, Codable, Hashable, Sendable {
    case text
    case multiline
    case choice
}

/// A form control requested by a template.
public struct FormFieldSpec: Codable, Hashable, Sendable {
    public var name: String
    public var kind: FormFieldKind
    /// Choices, for `choice` fields.
    public var options: [String]
    /// Written as `null` when unset.
    public var defaultValue: String?

    public init(name: String, kind: FormFieldKind, options: [String] = [], defaultValue: String? = nil) {
        self.name = name
        self.kind = kind
        self.options = options
        self.defaultValue = defaultValue
    }

    private enum CodingKeys: String, CodingKey { case name, kind, options, defaultValue }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(name, forKey: .name)
        try c.encode(kind, forKey: .kind)
        try c.encode(options, forKey: .options)
        try c.encodeNullable(defaultValue, forKey: .defaultValue)
    }
}

/// Rendered text, caret placement and diagnostics.
public struct RenderResult: Codable, Hashable, Sendable {
    public var text: String
    /// Spans over `text`, when the source was formatted (omitted when `nil`).
    public var formatting: [FormatSpan]?
    /// Caret offset in UTF-16 code units from the start of `text`, if a root `{{cursor}}` exists
    /// (written as `null` when absent).
    public var cursorOffset: Int?
    public var diagnostics: [Diagnostic]

    public init(text: String, formatting: [FormatSpan]? = nil, cursorOffset: Int? = nil, diagnostics: [Diagnostic] = []) {
        self.text = text
        self.formatting = formatting
        self.cursorOffset = cursorOffset
        self.diagnostics = diagnostics
    }

    private enum CodingKeys: String, CodingKey { case text, formatting, cursorOffset, diagnostics }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(text, forKey: .text)
        try c.encodeIfPresent(formatting, forKey: .formatting)
        try c.encodeNullable(cursorOffset, forKey: .cursorOffset)
        try c.encode(diagnostics, forKey: .diagnostics)
    }
}
