import Foundation

/// Parses v1 templates without throwing or discarding malformed source.
public enum TemplateParser {
    /// Returns source-located nodes and recoverable diagnostics.
    public static func parse(_ source: String) -> ParsedTemplate {
        let chars = Array(source)
        var offsets = [0]
        for c in chars { offsets.append(offsets.last! + c.utf16.count) }
        var nodes = [TemplateNode](), diagnostics = [TemplateDiagnostic]()
        var i = 0, textStart = 0, literal = "", hasCursor = false
        func flush(_ end: Int) {
            if !literal.isEmpty { nodes.append(.text(literal, range: offsets[textStart]..<offsets[end])); literal = "" }
        }
        while i < chars.count {
            if chars[i] == "\\", i + 1 < chars.count {
                if chars[i + 1] == "\\" { literal.append("\\"); i += 2; continue }
                if i + 2 < chars.count, chars[i + 1] == "{", chars[i + 2] == "{" {
                    literal += "{{"; i += 3; continue
                }
            }
            guard i + 1 < chars.count, chars[i] == "{", chars[i + 1] == "{" else {
                literal.append(chars[i]); i += 1; continue
            }
            // A tag cannot start with "{", so in "{{{" the first brace is literal text and the tag starts at the next.
            if i + 2 < chars.count, chars[i + 2] == "{" { literal.append("{"); i += 1; continue }
            let start = i
            var end = i + 2, quoted = false
            while end < chars.count {
                if quoted, chars[end] == "\\", end + 1 < chars.count { end += 2; continue }
                if chars[end] == "\"" { quoted.toggle() }
                if !quoted, end + 1 < chars.count, chars[end] == "}", chars[end + 1] == "}" { break }
                end += 1
            }
            guard end + 1 < chars.count else {
                literal += String(chars[start...])
                diagnostics.append(.init(severity: .error, message: "Unterminated tag", range: offsets[start]..<offsets[chars.count]))
                i = chars.count; break
            }
            let range = offsets[start]..<offsets[end + 2]
            // Unknown and malformed tags are kept as written so a newer version can still read them.
            func keepVerbatim(_ message: String) {
                flush(start)
                nodes.append(.tag(Tag(name: "", range: range, verbatim: String(chars[start..<(end + 2)]))))
                diagnostics.append(.init(severity: .error, message: message, range: range))
                textStart = end + 2
            }
            var scanner = TagScanner(chars: Array(chars[(start + 2)..<end]))
            if var tag = scanner.parse() {
                tag.range = range
                coerceBareTokens(&tag, bare: scanner.bareTokens)
                for name in scanner.duplicates {
                    diagnostics.append(.init(severity: .warning, message: "Duplicate attribute '\(name)'; last value wins", range: range))
                }
                if let problem = validate(tag) {
                    keepVerbatim(problem)
                } else {
                    flush(start)
                    nodes.append(.tag(tag))
                    if tag.name == "cursor" {
                        if hasCursor { diagnostics.append(.init(severity: .warning, message: "Only the first top-level cursor counts", range: range)) }
                        hasCursor = true
                    }
                    if ["date", "time"].contains(tag.name), tag.attributes.contains(where: { $0.name == "offset" }),
                       tag.string("offset").flatMap(DateOffset.init) == nil {
                        diagnostics.append(.init(severity: .warning, message: "Invalid date offset; ignored", range: range))
                    }
                    textStart = end + 2
                }
            } else {
                keepVerbatim("Malformed tag")
            }
            i = end + 2
        }
        flush(chars.count)
        return ParsedTemplate(nodes: nodes, diagnostics: diagnostics)
    }

    /// The string and boolean attributes of a known tag, or nil for an unknown name.
    private static func schema(_ name: String) -> (strings: [String], bools: [String])? {
        switch name {
        case "date", "time": (["format", "offset", "locale"], [])
        case "clipboard": ([], ["trim"])
        case "cursor": ([], [])
        case "snippet": (["id"], [])
        case "field": (["name", "default"], ["multiline"])
        case "choice": (["name", "options", "default"], [])
        default: nil
        }
    }

    /// A bare token given to a string attribute is that attribute's text, whatever type it would otherwise have:
    /// `default=5` is "5" and `default=007` is "007".
    private static func coerceBareTokens(_ tag: inout Tag, bare: [String: String]) {
        guard let schema = schema(tag.name) else { return }
        for index in tag.attributes.indices {
            let name = tag.attributes[index].name
            if schema.strings.contains(name), let token = bare[name] { tag.attributes[index].value = .string(token) }
        }
    }

    private static func validate(_ tag: Tag) -> String? {
        guard let (strings, bools) = schema(tag.name) else { return "Unknown tag '\(tag.name)'" }
        for attribute in tag.attributes {
            if strings.contains(attribute.name), case .string = attribute.value { continue }
            if bools.contains(attribute.name), case .bool = attribute.value { continue }
            if strings.contains(attribute.name) || bools.contains(attribute.name) { return "Invalid type for '\(attribute.name)'" }
        }
        if tag.name == "snippet", tag.string("id").flatMap(UUID.init(uuidString:)) == nil { return "snippet requires a UUID id" }
        if ["field", "choice"].contains(tag.name), tag.string("name") == nil { return "\(tag.name) requires name" }
        return nil
    }
}

private struct TagScanner {
    var chars: [Character]
    var i = 0
    var duplicates = [String]()
    /// Source text of attributes whose (last) value was written without quotes.
    var bareTokens = [String: String]()
    mutating func whitespace() { while i < chars.count, chars[i].isWhitespace { i += 1 } }
    mutating func identifier() -> String? {
        let start = i
        guard i < chars.count, ("a"..."z").contains(chars[i]) else { return nil }
        i += 1
        while i < chars.count, ("a"..."z").contains(chars[i]) || ("0"..."9").contains(chars[i]) || chars[i] == "_" { i += 1 }
        return String(chars[start..<i])
    }
    mutating func parse() -> Tag? {
        whitespace()
        guard let name = identifier() else { return nil }
        var attributes = [Attribute]()
        while i < chars.count {
            guard chars[i].isWhitespace else { return nil }
            whitespace()
            if i == chars.count { break }
            guard let key = identifier() else { return nil }
            whitespace()
            guard i < chars.count, chars[i] == "=" else { return nil }
            i += 1; whitespace()
            let valueStart = i
            guard let value = value() else { return nil }
            if attributes.contains(where: { $0.name == key }) {
                duplicates.append(key); attributes.removeAll { $0.name == key }
            }
            attributes.append(Attribute(name: key, value: value))
            bareTokens[key] = chars[valueStart] == "\"" ? nil : String(chars[valueStart..<i])
        }
        return Tag(name: name, attributes: attributes)
    }
    /// An optional sign, digits, and an optional fraction: `5`, `-3`, `+2.50`. No exponent, hex or bare `.5`.
    static func isPlainDecimal(_ token: String) -> Bool {
        var digits = Substring(token)
        if digits.first == "+" || digits.first == "-" { digits = digits.dropFirst() }
        let parts = digits.split(separator: ".", maxSplits: 1, omittingEmptySubsequences: false)
        return (1...2).contains(parts.count) && parts.allSatisfy { !$0.isEmpty && $0.allSatisfy { $0.isASCII && $0.isNumber } }
    }
    mutating func value() -> AttributeValue? {
        guard i < chars.count else { return nil }
        if chars[i] == "\"" {
            i += 1
            var result = ""
            while i < chars.count {
                if chars[i] == "\"" { i += 1; return .string(result) }
                if chars[i] == "\\" {
                    guard i + 1 < chars.count else { return nil }
                    let next = chars[i + 1]
                    guard next == "\\" || next == "\"" || next == "|" else { return nil }
                    if next == "|" { result.append("\\") }
                    result.append(next); i += 2
                } else { result.append(chars[i]); i += 1 }
            }
            return nil
        }
        let start = i
        while i < chars.count {
            let c = chars[i]
            guard c.isASCII && (c.isLetter || c.isNumber || "_.+-".contains(c)) else { break }
            i += 1
        }
        guard i > start else { return nil }
        let token = String(chars[start..<i])
        if token == "true" { return .bool(true) }
        if token == "false" { return .bool(false) }
        if Self.isPlainDecimal(token), let number = Double(token), number.isFinite { return .number(number) }
        return .string(token)
    }
}

/// Writes a deterministic canonical representation of template nodes.
public enum TemplateSerializer {
    /// Serializes nodes while protecting literal delimiters and preserving attribute types.
    ///
    /// Adjacent text is merged first, because `"{"` followed by `"{"` is the two-brace text `{{`.
    public static func serialize(_ nodes: [TemplateNode]) -> String {
        var merged = [TemplateNode]()
        for node in nodes {
            if case .text(let text, _) = node {
                if text.isEmpty { continue }
                if case .text(let previous, _)? = merged.last { merged[merged.count - 1] = .text(previous + text, range: 0..<0); continue }
            }
            merged.append(node)
        }
        return merged.map { node in
            switch node {
            case .text(let text, _):
                return escape(text)
            case .tag(let tag):
                if let verbatim = tag.verbatim { return verbatim }
                return "{{" + tag.name + tag.attributes.map { attribute in
                    let value: String
                    switch attribute.value {
                    case .string(let string): value = "\"" + string.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"") + "\""
                    case .bool(let bool): value = bool ? "true" : "false"
                    case .number(let number): value = plainDecimal(number)
                    }
                    return " " + attribute.name + "=" + value
                }.joined() + "}}"
            }
        }.joined()
    }

    /// Literal text: `\` becomes `\\` and each `{{` pair becomes `\{{`. A lone `{` needs no escape, even right
    /// before a tag, because the parser reads `{{{` as a literal brace followed by a tag.
    private static func escape(_ text: String) -> String {
        let chars = Array(text)
        var result = "", i = 0
        while i < chars.count {
            if chars[i] == "\\" { result += "\\\\" }
            else if chars[i] == "{", i + 1 < chars.count, chars[i + 1] == "{" { result += "\\{{"; i += 2; continue }
            else { result.append(chars[i]) }
            i += 1
        }
        return result
    }

    /// The shortest decimal text that parses back to `value`, without an exponent (a bare `1e5` is a string).
    private static func plainDecimal(_ value: Double) -> String {
        let description = "\(value)"
        guard let marker = description.firstIndex(where: { $0 == "e" || $0 == "E" }), let exponent = Int(description[description.index(after: marker)...]) else {
            return description.hasSuffix(".0") ? String(description.dropLast(2)) : description
        }
        var mantissa = String(description[..<marker])
        let negative = mantissa.hasPrefix("-")
        if negative { mantissa.removeFirst() }
        let point = mantissa.firstIndex(of: ".").map { mantissa.distance(from: mantissa.startIndex, to: $0) } ?? mantissa.count
        let digits = mantissa.replacingOccurrences(of: ".", with: "")
        let position = point + exponent
        var text: String
        if position <= 0 { text = "0." + String(repeating: "0", count: -position) + digits }
        else if position >= digits.count { text = digits + String(repeating: "0", count: position - digits.count) }
        else { text = String(digits.prefix(position)) + "." + String(digits.dropFirst(position)) }
        if text.contains(".") { while text.hasSuffix("0") { text.removeLast() }; if text.hasSuffix(".") { text.removeLast() } }
        return (negative ? "-" : "") + text
    }
}
