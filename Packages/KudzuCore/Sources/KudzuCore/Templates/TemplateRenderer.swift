import Foundation

/// Explicit inputs for deterministic template expansion.
public struct RenderContext: Sendable {
    /// The instant used by all date/time tags, including nested templates.
    public var now: Date
    /// Calendar arithmetic and formatting calendar.
    public var calendar: Calendar
    /// Time zone applied to arithmetic and formatting.
    public var timeZone: TimeZone
    /// Default formatting locale.
    public var locale: Locale
    /// Plain-text clipboard snapshot.
    public var clipboard: String?
    /// Form values keyed by field name.
    public var formValues: [String: String]
    /// Resolves an ID to template source, never pre-rendered text.
    public var resolveSnippet: @Sendable (UUID) -> String?
    /// The snippet being rendered, if known; a reference to it is a cycle even at the top level.
    public var rootSnippetID: UUID?
    /// Creates a rendering context with explicit temporal inputs.
    public init(now: Date, calendar: Calendar, timeZone: TimeZone, locale: Locale,
                clipboard: String? = nil, formValues: [String: String] = [:],
                resolveSnippet: @escaping @Sendable (UUID) -> String? = { _ in nil },
                rootSnippetID: UUID? = nil) {
        self.now = now; self.calendar = calendar; self.timeZone = timeZone; self.locale = locale
        self.clipboard = clipboard; self.formValues = formValues; self.resolveSnippet = resolveSnippet
        self.rootSnippetID = rootSnippetID
    }
}

/// Rendered text, caret placement, and recoverable issues.
public struct RenderResult: Equatable, Sendable {
    /// Expanded plain text.
    public var text: String
    /// Caret offset in UTF-16 code units, if specified at the top level.
    public var cursorOffset: Int?
    /// Diagnostics whose ranges refer to their containing template sources.
    public var diagnostics: [TemplateDiagnostic]
}

/// Expands templates recursively with path-based cycle detection.
public enum TemplateRenderer {
    /// Renders source with a maximum of eight nested snippet levels.
    public static func render(_ source: String, context: RenderContext) -> RenderResult {
        var state = Renderer(context: context)
        state.render(source, path: context.rootSnippetID.map { [$0] } ?? [], depth: 0)
        return state.result
    }
}

private struct Renderer {
    var context: RenderContext
    var result = RenderResult(text: "", cursorOffset: nil, diagnostics: [])
    var utf16Count = 0
    var formatters: [FormatKey: DateFormatter] = [:]
    struct FormatKey: Hashable { var format: String; var locale: String }

    mutating func append(_ text: String) { result.text += text; utf16Count += text.utf16.count }
    mutating func render(_ source: String, path: Set<UUID>, depth: Int) {
        let parsed = TemplateParser.parse(source)
        result.diagnostics += parsed.diagnostics
        for node in parsed.nodes {
            switch node {
            case .text(let text, _): append(text)
            case .tag(let tag):
                switch tag.typed {
                case .cursor:
                    if depth == 0, result.cursorOffset == nil { result.cursorOffset = utf16Count }
                case .clipboard(let trim):
                    let text = context.clipboard ?? ""
                    append(trim ? text.trimmingCharacters(in: .whitespacesAndNewlines) : text)
                case .field(let name, let value, _), .choice(let name, _, let value):
                    append(context.formValues[name] ?? value ?? "")
                case .snippet(let id):
                    if path.contains(id) { diagnose("Snippet cycle detected", tag) }
                    else if depth >= 8 { diagnose("Maximum snippet depth of 8 exceeded", tag) }
                    else if let nested = context.resolveSnippet(id) { render(nested, path: path.union([id]), depth: depth + 1) }
                    else { diagnose("Missing snippet \(id)", tag) }
                case .date(let format, let offset, let locale), .time(let format, let offset, let locale):
                    var calendar = context.calendar
                    calendar.timeZone = context.timeZone
                    var date = context.now
                    if let offset {
                        if let adjusted = offset.applying(to: date, calendar: calendar) { date = adjusted }
                        else { diagnose("Date offset could not be applied; ignored", tag) }
                    }
                    let key = FormatKey(format: format, locale: locale ?? context.locale.identifier)
                    let formatter: DateFormatter
                    if let existing = formatters[key] { formatter = existing }
                    else {
                        formatter = DateFormatter()
                        formatter.locale = Locale(identifier: key.locale)
                        formatter.calendar = calendar
                        formatter.timeZone = context.timeZone
                        formatter.dateFormat = format
                        formatters[key] = formatter
                    }
                    append(formatter.string(from: date))
                case .unknown: append(TemplateSerializer.serialize([node]))  // verbatim source for unrecognized tags
                }
            }
        }
    }
    mutating func diagnose(_ message: String, _ tag: Tag) {
        result.diagnostics.append(.init(severity: .warning, message: message, range: tag.range))
    }
}
