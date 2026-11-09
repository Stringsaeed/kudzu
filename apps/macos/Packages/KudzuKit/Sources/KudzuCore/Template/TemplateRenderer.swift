import Foundation

/// Explicit inputs for deterministic rendering. Snippet resolvers return source, never rendered text.
public struct RenderContext: Sendable {
    public var now: Date
    public var calendar: Calendar
    public var timeZone: TimeZone
    public var locale: Locale
    public var clipboard: String?
    public var formValues: [String: String]
    public var resolveSnippet: @Sendable (UUID) -> String?
    public var resolveFormatting: (@Sendable (UUID) -> Formatting?)?
    public var rootSnippetID: UUID?
    /// Optional exact fraction for RFC 3339 hosts (Date itself has floating-point precision).
    public var nanosecond: Int?
    public init(now: Date, calendar: Calendar = Calendar(identifier: .gregorian), timeZone: TimeZone = .gmt,
                locale: Locale = Locale(identifier: "en_US"), clipboard: String? = nil, formValues: [String: String] = [:],
                resolveSnippet: @escaping @Sendable (UUID) -> String? = { _ in nil },
                resolveFormatting: (@Sendable (UUID) -> Formatting?)? = nil, rootSnippetID: UUID? = nil, nanosecond: Int? = nil) {
        self.now = now; self.calendar = calendar; self.timeZone = timeZone; self.locale = locale
        self.clipboard = clipboard; self.formValues = formValues; self.resolveSnippet = resolveSnippet
        self.resolveFormatting = resolveFormatting; self.rootSnippetID = rootSnippetID; self.nanosecond = nanosecond
    }
}

/// Expands templates recursively with active-path cycle detection and eight nested snippet levels.
public enum TemplateRenderer {
    public static func render(_ source: String, context: RenderContext) -> RenderResult { renderFormatted(source, formatting: nil, context: context) }
    public static func renderFormatted(_ source: String, formatting: Formatting?, context: RenderContext) -> RenderResult {
        let formatting = formatting.flatMap { $0.source.utf16.elementsEqual(source.utf16) ? $0 : nil }
        var state = Renderer(context: context, result: RenderResult(text: "", formatting: formatting.map { _ in [] }, cursorOffset: nil, diagnostics: []))
        state.render(source, path: context.rootSnippetID.map { [$0] } ?? [], depth: 0, formatting: formatting)
        return state.result
    }
}

private struct Renderer {
    var context: RenderContext
    var result: RenderResult
    var utf16Count = 0
    /// `formValues` keyed by bytes, like Rust's `HashMap<String, String>`: canonically equivalent names stay distinct.
    lazy var formValues: [[UInt8]: String] = Dictionary(context.formValues.map { ($0.key.exactKey, $0.value) }, uniquingKeysWith: { first, _ in first })
    mutating func append(_ text: String) { result.text += text; utf16Count += text.utf16.count }
    mutating func render(_ source: String, path: Set<UUID>, depth: Int, formatting: Formatting?) {
        var positions: [Int: Int] = [:]
        let parsed = TemplateParser.parse(source)
        result.diagnostics += parsed.diagnostics
        for node in parsed.nodes {
            switch node {
            case .text(let text, let range):
                if formatting != nil { FormattingHelpers.literalPositions(FormattingHelpers.substring(source, range), start: range.lowerBound, output: utf16Count, positions: &positions) }
                append(text)
            case .tag(let tag):
                if formatting != nil { positions[tag.range.lowerBound] = utf16Count }
                switch tag.typed {
                case .cursor: if depth == 0, result.cursorOffset == nil { result.cursorOffset = utf16Count }
                case .clipboard(let trim):
                    let text = context.clipboard ?? ""
                    append(trim ? text.trimmingCharacters(in: .whitespacesAndNewlines) : text)
                case .field(let name, let value, _), .choice(let name, _, let value): append(formValues[name.exactKey] ?? value ?? "")
                case .snippet(let id):
                    if path.contains(id) { diagnose("Snippet cycle detected", tag) }
                    else if depth >= 8 { diagnose("Maximum snippet depth of 8 exceeded", tag) }
                    else if let nested = context.resolveSnippet(id) {
                        let rich = result.formatting == nil ? nil : context.resolveFormatting?(id).flatMap { $0.source.utf16.elementsEqual(nested.utf16) ? $0 : nil }
                        render(nested, path: path.union([id]), depth: depth + 1, formatting: rich)
                    } else { diagnose("Missing snippet \(id.uuidString)", tag) }
                case .date(let format, let offset, let locale), .time(let format, let offset, let locale):
                    let fixedZone = TimeZone(secondsFromGMT: context.timeZone.secondsFromGMT(for: context.now)) ?? .gmt
                    var date = context.now
                    if let offset {
                        if let adjusted = offset.apply(to: date, timeZone: fixedZone) { date = adjusted }
                        else { diagnose("Date offset could not be applied; ignored", tag) }
                    }
                    append(Template.formatICU(date, pattern: format, locale: locale ?? context.locale.identifier, timeZone: fixedZone, nanosecond: context.nanosecond))
                case .unknown: append(TemplateSerializer.serialize([node]))
                }
                if formatting != nil { positions[tag.range.upperBound] = utf16Count }
            }
        }
        if let formatting {
            result.formatting?.append(contentsOf: formatting.spans.compactMap { span in
                guard let start = positions[span.start], let end = positions[span.end], start < end else { return nil }
                return FormatSpan(start: start, end: end, style: span.style)
            })
        }
    }
    mutating func diagnose(_ message: String, _ tag: Tag) { result.diagnostics.append(.init(severity: .warning, message: message, range: tag.range)) }
}

/// Template syntax, canonical serialization, rendering, analysis and editor helpers.
public enum Template {
    public static func parse(_ source: String) -> ParsedTemplate { TemplateParser.parse(source) }
    public static func serialize(_ nodes: [TemplateNode]) -> String { TemplateSerializer.serialize(nodes) }
    public static func typed(_ tag: Tag) -> TemplateTag { tag.typed }
    public static func render(_ source: String, context: RenderContext) -> RenderResult { TemplateRenderer.render(source, context: context) }
    public static func renderFormatted(_ source: String, formatting: Formatting?, context: RenderContext) -> RenderResult { TemplateRenderer.renderFormatted(source, formatting: formatting, context: context) }
    public static func analyze(_ parsed: ParsedTemplate) -> TemplateInfo { TemplateAnalysis.analyze(parsed) }
    public static func formFields(_ source: String, resolveSnippet: (UUID) -> String?) -> [FormFieldSpec] { TemplateAnalysis.formFields(in: source, resolveSnippet: resolveSnippet) }
    public static func splitOptions(_ source: String) -> [String] {
        guard !source.isEmpty else { return [] }
        let chars = source.unicodeScalars.map(String.init); var out: [String] = [], item = "", i = 0
        while i < chars.count {
            if chars[i] == "\\", i + 1 < chars.count, chars[i + 1] == "|" { item += "|"; i += 2 }
            else if chars[i] == "|" { out.append(item); item = ""; i += 1 }
            else { item += chars[i]; i += 1 }
        }
        return out + [item]
    }
}
