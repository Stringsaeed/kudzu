import Foundation

/// Formatting annotates the unchanged plain-text template using UTF-16 ranges.
public enum FormattingHelpers {
    /// `http(s)://` or `mailto:` (ASCII case-insensitive, compared by scalar like `kudzu-core`, so a combining
    /// mark after the prefix does not hide it) with no whitespace, control characters, quotes or angle brackets.
    ///
    /// Whitespace and controls follow Rust `char::is_whitespace` / `is_control`, which the
    /// `template-formatting.json` fixtures lock. That accepts U+FEFF and rejects U+0085 and the
    /// other C1 controls. JavaScript `\s` does the opposite for those two scalars. Stored and
    /// imported spans keep this check. A caller that validates editor input can pass a different
    /// predicate to ``validSpans(_:_:isValidLink:)``.
    public static func safeLink(_ url: String) -> Bool {
        let lower = url.unicodeScalars.lazy.map(\.asciiLowercased)
        return ["https://", "http://", "mailto:"].contains { lower.starts(with: $0.unicodeScalars) } &&
            !url.unicodeScalars.contains { Character(String($0)).isWhitespace || $0.value <= 31 || (127...159).contains($0.value) || "\"'<>".unicodeScalars.contains($0) }
    }
    /// Raster data only: no remote fetches or active SVG content in imported snippets.
    public static func safeImage(_ url: String) -> Bool {
        guard url.utf8.count <= 196_608 else { return false }
        let prefixes = [("data:image/png;base64,", "iVBORw0KGgo"), ("data:image/jpeg;base64,", "/9j/")]
        for (prefix, magic) in prefixes where url.hasPrefix(prefix) {
            let data = url.dropFirst(prefix.count)
            return data.hasPrefix(magic) && data.utf8.count % 4 == 0 && data.utf8.allSatisfy { (65...90).contains($0) || (97...122).contains($0) || (48...57).contains($0) || [43, 47, 61].contains($0) }
        }
        return false
    }
    /// `isValidLink` filters link spans. The default is the Rust library rule (``safeLink(_:)``).
    /// Editor input may pass its own predicate; import and storage keep the default.
    public static func validSpans(_ text: String, _ spans: [FormatSpan], isValidLink: (String) -> Bool = FormattingHelpers.safeLink) -> [FormatSpan] {
        var boundaries: Set<Int> = [0]; var offset = 0
        for scalar in text.unicodeScalars { offset += scalar.utf16.count; boundaries.insert(offset) }
        return spans.filter { span in
            guard span.start < span.end, boundaries.contains(span.start), boundaries.contains(span.end) else { return false }
            switch span.style {
            case .link(let url): return isValidLink(url)
            case .image(let dataUrl, let width): return safeImage(dataUrl) && (1...512).contains(width)
            default: return true
            }
        }
    }
    public static func sourceSpans(_ content: String, formatting: Formatting?) -> [FormatSpan]? {
        guard let formatting, formatting.source.utf16.elementsEqual(content.utf16) else { return nil }
        return validSpans(content, formatting.spans)
    }
    public static func sameStyle(_ a: FormatStyle, _ b: FormatStyle) -> Bool {
        switch (a,b) {
        case (.bold,.bold),(.italic,.italic),(.underline,.underline): return true
        case (.link(let a),.link(let b)): return a.utf16.elementsEqual(b.utf16)
        case (.image(let a,let aw),.image(let b,let bw)): return aw == bw && a.utf16.elementsEqual(b.utf16)
        default: return false
        }
    }
    public static func sameSpans(_ a: [FormatSpan]?, _ b: [FormatSpan]?) -> Bool {
        guard let a, let b else { return a == nil && b == nil }
        return a.count == b.count && zip(a,b).allSatisfy { $0.start == $1.start && $0.end == $1.end && sameStyle($0.style,$1.style) }
    }
    public static func substring(_ text: String, _ range: Range<Int>) -> String {
        String(decoding: Array(text.utf16)[range], as: UTF16.self)
    }
    /// Each image occupies one attachment character in the rich paste; alternatives remain in the plain flavor.
    public static func insertionText(_ text: String, spans: [FormatSpan]) -> String {
        var out = "", at = 0
        for image in images(spans) {
            guard image.start >= at, image.start < image.end, image.end <= text.utf16.count else { continue }
            out += substring(text, at..<image.start) + "\u{fffc}"; at = image.end
        }
        return out + substring(text, at..<text.utf16.count)
    }
    public static func insertionOffset(_ offset: Int, spans: [FormatSpan]) -> Int {
        var removed = 0, end = 0
        for image in images(spans) {
            guard image.start >= end, image.start < image.end, image.start < offset else { continue }
            removed += min(offset, image.end) - image.start - 1; end = image.end
        }
        return max(0, offset - removed)
    }
    private static func images(_ spans: [FormatSpan]) -> [FormatSpan] {
        spans.filter { if case .image(let url, _) = $0.style { return safeImage(url) }; return false }.sorted { $0.start < $1.start }
    }
    /// Escapes `& < > " '` scalar by scalar, so one carrying a combining mark is escaped too (as in `kudzu-core`;
    /// otherwise `"` + U+0301 in an image's alt text would end the attribute).
    public static func escapeHTML(_ text: String) -> String {
        var result = ""
        for scalar in text.unicodeScalars {
            switch scalar {
            case "&": result += "&amp;"
            case "<": result += "&lt;"
            case ">": result += "&gt;"
            case "\"": result += "&quot;"
            case "'": result += "&#39;"
            default: result.unicodeScalars.append(scalar)
            }
        }
        return result
    }
    public struct Part: Sendable, Hashable {
        public var start: Int
        public var text: String
        public var image: FormatSpan?
        public var styles: [FormatStyle]
    }
    /// Splits overlaps into independently styled pieces without interpreting HTML.
    public static func formattedParts(_ text: String, spans: [FormatSpan] = [], cursor: Int? = nil) -> [Part] {
        let valid = validSpans(text, spans)
        var boundaries = Set([0, text.utf16.count] + valid.flatMap { [$0.start, $0.end] })
        if let cursor, (0...text.utf16.count).contains(cursor) { boundaries.insert(cursor) }
        let sorted = boundaries.sorted()
        return zip(sorted, sorted.dropFirst()).map { start, end in
            Part(start: start, text: substring(text, start..<end), image: valid.first { if case .image = $0.style { return $0.start <= start && $0.end > start }; return false }, styles: valid.filter { $0.start <= start && $0.end >= end }.map(\.style))
        }
    }
    /// Only owned markup is emitted; template values and imported URLs cannot add markup.
    public static func html(_ text: String, spans: [FormatSpan]) -> String {
        var output = "<html><head><meta charset=\"utf-8\"></head><body><span style=\"white-space: pre-wrap; font-family: -apple-system, sans-serif;\">"
        for part in formattedParts(text, spans: spans) {
            if let image = part.image, case .image(let url, let width) = image.style {
                if image.start == part.start { output += "<img src=\"\(url)\" width=\"\(width)\" alt=\"\(escapeHTML(substring(text, image.start..<image.end)))\">" }
                continue
            }
            var value = escapeHTML(part.text).replacingOccurrences(of: "\n", with: "<br>")
            if part.styles.contains(.underline) { value = "<u>\(value)</u>" }
            if part.styles.contains(.italic) { value = "<em>\(value)</em>" }
            if part.styles.contains(.bold) { value = "<strong>\(value)</strong>" }
            if let link = part.styles.first(where: { if case .link = $0 { true } else { false } }), case .link(let url) = link { value = "<a href=\"\(escapeHTML(url))\">\(value)</a>" }
            output += value
        }
        return output + "</span></body></html>"
    }
    /// Match the parser's grapheme-aware escapes. Tag interiors deliberately have no position.
    public static func literalPositions(_ raw: String, start: Int, output: Int, positions: inout [Int: Int]) {
        let chars = Array(raw); var i = 0, input = start, rendered = output
        positions[input] = rendered
        while i < chars.count {
            if chars[i] == "\\", i + 1 < chars.count, chars[i + 1] == "\\" || (chars[i + 1] == "{" && i + 2 < chars.count && chars[i + 2] == "{") { input += 1; i += 1 }
            let count = chars[i].utf16.count
            input += count; rendered += count; positions[input] = rendered; i += 1
        }
    }
}

private extension Unicode.Scalar {
    var asciiLowercased: Unicode.Scalar { (65...90).contains(value) ? Unicode.Scalar(UInt8(value + 32)) : self }
}

// The namespace is also available on the model for editor clients.
extension Formatting {
    public static func validSpans(_ text: String, spans: [FormatSpan], isValidLink: (String) -> Bool = FormattingHelpers.safeLink) -> [FormatSpan] {
        FormattingHelpers.validSpans(text, spans, isValidLink: isValidLink)
    }
    public static func sameStyle(_ a: FormatStyle, _ b: FormatStyle) -> Bool { FormattingHelpers.sameStyle(a,b) }
    public static func escapeHTML(_ text: String) -> String { FormattingHelpers.escapeHTML(text) }
    public static func formattedHtml(_ text: String, spans: [FormatSpan]) -> String { FormattingHelpers.html(text,spans:spans) }
    public static func formattedParts(_ text: String, spans: [FormatSpan] = [], cursor: Int? = nil) -> [FormattingHelpers.Part] { FormattingHelpers.formattedParts(text,spans:spans,cursor:cursor) }
    public static func safeLink(_ url: String) -> Bool { FormattingHelpers.safeLink(url) }
    public static func safeImage(_ url: String) -> Bool { FormattingHelpers.safeImage(url) }
    public static func sourceSpans(_ content: String, formatting: Formatting?) -> [FormatSpan]? { FormattingHelpers.sourceSpans(content, formatting: formatting) }
    public static func sameSpans(_ a: [FormatSpan]?, _ b: [FormatSpan]?) -> Bool { FormattingHelpers.sameSpans(a,b) }
    public static func html(_ text: String, spans: [FormatSpan]) -> String { FormattingHelpers.html(text, spans: spans) }
    public static func insertionText(_ text: String, spans: [FormatSpan]) -> String { FormattingHelpers.insertionText(text, spans: spans) }
    public static func insertionOffset(_ offset: Int, spans: [FormatSpan]) -> Int { FormattingHelpers.insertionOffset(offset, spans: spans) }
}
