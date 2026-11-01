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
            var scanner = TagScanner(chars: Array(chars[(start + 2)..<end]))
            if var tag = scanner.parse() {
                tag.range = range
                for name in scanner.duplicates {
                    diagnostics.append(.init(severity: .warning, message: "Duplicate attribute '\(name)'; last value wins", range: range))
                }
                if let problem = validate(tag) {
                    literal += String(chars[start..<(end + 2)])
                    diagnostics.append(.init(severity: .error, message: problem, range: range))
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
                literal += String(chars[start..<(end + 2)])
                diagnostics.append(.init(severity: .error, message: "Malformed tag", range: range))
            }
            i = end + 2
        }
        flush(chars.count)
        return ParsedTemplate(nodes: nodes, diagnostics: diagnostics)
    }

    private static func validate(_ tag: Tag) -> String? {
        let strings: [String]
        let bools: [String]
        switch tag.name {
        case "date", "time": strings = ["format", "offset", "locale"]; bools = []
        case "clipboard": strings = []; bools = ["trim"]
        case "cursor": strings = []; bools = []
        case "snippet": strings = ["id"]; bools = []
        case "field": strings = ["name", "default"]; bools = ["multiline"]
        case "choice": strings = ["name", "options", "default"]; bools = []
        default: return "Unknown tag '\(tag.name)'"
        }
        for attribute in tag.attributes {
            if ["date", "time"].contains(tag.name), attribute.name == "offset" { continue }
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
            guard let value = value() else { return nil }
            if attributes.contains(where: { $0.name == key }) {
                duplicates.append(key); attributes.removeAll { $0.name == key }
            }
            attributes.append(Attribute(name: key, value: value))
        }
        return Tag(name: name, attributes: attributes)
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
        if let number = Double(token), number.isFinite { return .number(number) }
        return .string(token)
    }
}

/// Writes a deterministic canonical representation of template nodes.
public enum TemplateSerializer {
    /// Serializes nodes while protecting literal delimiters and preserving attribute types.
    public static func serialize(_ nodes: [TemplateNode]) -> String {
        nodes.map { node in
            switch node {
            case .text(let text, _):
                return text.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "{{", with: "\\{{")
            case .tag(let tag):
                return "{{" + tag.name + tag.attributes.map { attribute in
                    let value: String
                    switch attribute.value {
                    case .string(let string): value = "\"" + string.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"") + "\""
                    case .bool(let bool): value = bool ? "true" : "false"
                    case .number(let number): value = String(number)
                    }
                    return " " + attribute.name + "=" + value
                }.joined() + "}}"
            }
        }.joined()
    }
}
