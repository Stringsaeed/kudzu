import Foundation

public struct TagToken: Hashable, Sendable {
    public var from: Int
    public var to: Int
    public var source: String
    public var name: String
    public var attrs: [String: AttributeValue]
    public var verbatim: Bool
}
public enum TemplateToken: Hashable, Sendable {
    case text(text: String, from: Int, to: Int)
    case tag(TagToken)
}
public enum ChipTone: String, Sendable { case time, clipboard, cursor, snippet, form, error }

extension Template {
    public static let KNOWN_TAGS = ["date", "time", "clipboard", "cursor", "snippet", "field", "choice"]
    public static let knownTags = KNOWN_TAGS
    public static let dateFormatPresets: [(format: String, label: String)] = [
        ("yyyy-MM-dd", "ISO date"), ("MMMM d, yyyy", "Long date"), ("MMM d, yyyy", "Medium date"),
        ("M/d/yy", "Short date"), ("d MMMM yyyy", "Day month year"), ("EEEE", "Weekday"),
        ("EEEE, MMMM d", "Weekday + date"), ("HH:mm", "24-hour time"), ("h:mm a", "12-hour time"),
        ("HH:mm:ss", "Time with seconds"), ("yyyy-MM-dd'T'HH:mm:ssXXX", "ISO timestamp")
    ]
    /// Client-side tokenizer from template.ts. Its permissive attribute scanner is intentionally separate
    /// from the insertion parser: unknown quoted escapes and uppercase attribute keys survive editing.
    public static func tokenize(_ source: String) -> [TemplateToken] {
        let chars = source.unicodeScalars.map(String.init)
        var offsets = [0]; for c in chars { offsets.append((offsets.last ?? 0) + c.utf16.count) }
        var i = 0, textFrom = 0, text = "", out: [TemplateToken] = []
        func flush(_ at: Int) { if !text.isEmpty { out.append(.text(text: text, from: offsets[textFrom], to: offsets[at])); text = "" } }
        while i < chars.count {
            if chars[i] == "\\", i + 1 < chars.count, chars[i + 1] == "\\" { text += "\\"; i += 2; continue }
            if chars[i] == "\\", i + 2 < chars.count, chars[i + 1] == "{", chars[i + 2] == "{" { text += "{{"; i += 3; continue }
            if chars[i] == "{", i + 1 < chars.count, chars[i + 1] == "{", (i + 2 == chars.count || chars[i + 2] != "{") {
                var end = i + 2, quoted = false
                while end < chars.count {
                    if quoted, chars[end] == "\\", end + 1 < chars.count { end += 2; continue }
                    if chars[end] == "\"" { quoted.toggle() }
                    if !quoted, chars[end] == "}", end + 1 < chars.count, chars[end + 1] == "}" { break }
                    end += 1
                }
                if end + 1 >= chars.count { text += "{{"; i += 2; continue }
                flush(i)
                let raw = chars[i..<(end + 2)].joined()
                var scanner = EditorTagScanner(chars: Array(chars[(i + 2)..<end]))
                let parsed = scanner.parse()
                out.append(.tag(TagToken(from: offsets[i], to: offsets[end + 2], source: raw, name: parsed?.name ?? "", attrs: parsed?.attrs ?? [:], verbatim: parsed == nil)))
                i = end + 2; textFrom = i; continue
            }
            text += chars[i]; i += 1
        }
        flush(chars.count); return out
    }
    public static func buildTag(_ name: String, attrs: [Attribute] = []) -> String {
        TemplateSerializer.serialize([.tag(Tag(name: name, attributes: attrs.filter { $0.value != .string("") }))])
    }
    public static func escapeText(_ text: String) -> String { text.replacingOccurrences(of:"\\",with:"\\\\").replacingOccurrences(of:"{{",with:"\\{{") }
    /// Escapes every `|` scalar (also one carrying a combining mark, which `splitOptions` still splits on).
    public static func joinOptions(_ options: [String]) -> String {
        options.map { option in
            var escaped = String.UnicodeScalarView()
            for scalar in option.unicodeScalars {
                if scalar == "|" { escaped.append("\\") }
                escaped.append(scalar)
            }
            return String(escaped)
        }.joined(separator: "|")
    }
    public static func chipLabel(_ tag: TagToken, snippetName: ((String) -> String?)? = nil) -> String {
        if tag.verbatim { return tag.source }
        let a = tag.attrs
        func str(_ key: String) -> String? { if case .string(let v) = a[key] { return v }; return nil }
        switch tag.name {
        case "date", "time":
            let format = str("format") ?? (tag.name == "date" ? "yyyy-MM-dd" : "HH:mm")
            let base = dateFormatPresets.first { $0.format.isExactly(format) }?.label ?? format
            return (tag.name == "date" ? "Date" : "Time") + " · " + base + (str("offset").flatMap { $0.isEmpty ? nil : " " + $0 } ?? "")
        case "clipboard": return a["trim"] == .bool(true) ? "Clipboard (trimmed)" : "Clipboard"
        case "cursor": return "Cursor"
        case "snippet": return "↳ " + (str("id").flatMap { snippetName?($0) } ?? "Missing snippet")
        case "field": return (str("name") ?? "") + (a["multiline"] == .bool(true) ? " ¶" : "")
        case "choice": return (str("name") ?? "") + " ▾"
        default: return tag.source
        }
    }
    public static func chipTone(_ tag: TagToken) -> ChipTone {
        if tag.verbatim { return .error }
        switch tag.name {
        case "date", "time": return .time
        case "clipboard": return .clipboard
        case "cursor": return .cursor
        case "snippet": return .snippet
        default: return .form
        }
    }
    /// One-line summary for list rows, with tags shown as labels.
    public static func plainSummary(_ source: String, snippetName: ((String) -> String?)? = nil, max: Int = 140) -> String {
        var out = ""
        for t in tokenize(source) {
            switch t { case .text(let text, _, _): out += text; case .tag(let tag): out += "‹" + chipLabel(tag, snippetName: snippetName) + "›" }
            if out.utf16.count > max { break }
        }
        let collapsed = ECMAScriptWhitespace.collapse(out)
        return String(decoding: collapsed.utf16.prefix(Swift.max(0, max)), as: UTF16.self)
    }
}

private struct EditorTagScanner {
    var chars: [String]; var i = 0
    /// JavaScript `\s`, as `parseInner` in template.ts.
    mutating func ws() { while i < chars.count, chars[i].unicodeScalars.allSatisfy(ECMAScriptWhitespace.contains) { i += 1 } }
    func allowed(_ char: String, _ set: String) -> Bool { char.utf8.count == 1 && set.contains(char) }
    mutating func parse() -> (name: String, attrs: [String: AttributeValue])? {
        let lower = "abcdefghijklmnopqrstuvwxyz", digits = "0123456789"
        ws(); guard i < chars.count, allowed(chars[i], lower) else { return nil }
        let start = i; while i < chars.count, allowed(chars[i], lower + digits + "_") { i += 1 }
        let name = chars[start..<i].joined(); var attrs: [String: AttributeValue] = [:]
        while i < chars.count {
            let before = i; ws(); if i == chars.count { break }; guard i != before else { return nil }
            let keyStart = i
            while i < chars.count, allowed(chars[i], lower + lower.uppercased() + digits + "_") { i += 1 }
            guard i > keyStart else { return nil }; let key = chars[keyStart..<i].joined()
            ws(); guard i < chars.count, chars[i] == "=" else { return nil }; i += 1; ws(); guard i < chars.count else { return nil }
            if chars[i] == "\"" {
                i += 1; var value = "", closed = false
                while i < chars.count {
                    if chars[i] == "\\", i + 1 < chars.count, ["\\", "\""].contains(chars[i + 1]) { value += chars[i + 1]; i += 2 }
                    else if chars[i] == "\"" { i += 1; closed = true; break }
                    else { value += chars[i]; i += 1 }
                }
                guard closed else { return nil }; attrs[key] = .string(value)
            } else {
                let start = i
                while i < chars.count, allowed(chars[i], lower + lower.uppercased() + digits + "_.+-") { i += 1 }
                guard i > start else { return nil }; let bare = chars[start..<i].joined()
                if ["default", "name", "format", "options", "offset", "locale", "id"].contains(key) { attrs[key] = .string(bare) }
                else if bare == "true" || bare == "false" { attrs[key] = .bool(bare == "true") }
                else if bare.range(of: #"^[+-]?\d+(\.\d+)?$"#, options: .regularExpression) != nil, let n = Double(bare) { attrs[key] = .number(n) }
                else { attrs[key] = .string(bare) }
            }
        }
        func str(_ key: String, required: Bool = false) -> Bool { guard let v = attrs[key] else { return !required }; if case .string = v { return true }; return false }
        func bool(_ key: String) -> Bool { guard let v = attrs[key] else { return true }; if case .bool = v { return true }; return false }
        let valid: Bool
        switch name {
        case "date", "time": valid = str("format") && str("locale")
        case "clipboard": valid = bool("trim")
        case "cursor": valid = true
        case "snippet": if case .string(let id) = attrs["id"] { valid = id.utf8.count == 36 && UUID(uuidString: id) != nil } else { valid = false }
        case "field": valid = str("name", required: true) && str("default") && bool("multiline")
        case "choice": valid = str("name", required: true) && str("options") && str("default")
        default: valid = false
        }
        return valid ? (name, attrs) : nil
    }
}

public struct TemplateFacts: Hashable, Sendable {
    public var isForm = false
    public var fieldNames: [String] = []
    public var usesClipboard = false
    public var hasCursor = false
    public var snippetIds: [String] = []
    public var hasErrors = false
}
extension Template {
    /// Facts for the chip editor, using the client tokenizer's permissive grammar.
    public static func analyze(_ source: String) -> TemplateFacts {
        var facts = TemplateFacts()
        for token in tokenize(source) {
            guard case .tag(let tag) = token else { continue }
            if tag.verbatim { facts.hasErrors = true }
            else if tag.name == "field" || tag.name == "choice" {
                facts.isForm = true
                if case .string(let name) = tag.attrs["name"], !facts.fieldNames.contains(where: { $0.utf16.elementsEqual(name.utf16) }) { facts.fieldNames.append(name) }
            } else if tag.name == "clipboard" { facts.usesClipboard = true }
            else if tag.name == "cursor" { facts.hasCursor = true }
            else if tag.name == "snippet", case .string(let id) = tag.attrs["id"] { facts.snippetIds.append(id) }
        }
        return facts
    }
}
