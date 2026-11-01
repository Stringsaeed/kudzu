import Foundation

/// Formats and scans Kudzu template strings without depending on the `Templates/` parser module.
enum TemplateTagFormatter {
    /// Escapes literal text so that any `{{` is written as `\{{`.
    static func escapeLiteral(_ text: String) -> String {
        text.replacingOccurrences(of: "{{", with: "\\{{")
    }

    /// Unescapes `\{{` in literal template segments back to `{{`.
    static func unescapeLiteral(_ text: String) -> String {
        text.replacingOccurrences(of: "\\{{", with: "{{")
    }

    /// Quotes and escapes an attribute string value (`\` -> `\\`, `"` -> `\"`).
    static func quoteAttribute(_ value: String) -> String {
        var escaped = ""
        escaped.reserveCapacity(value.count + 2)
        escaped.append("\"")
        for ch in value {
            if ch == "\\" {
                escaped += "\\\\"
            } else if ch == "\"" {
                escaped += "\\\""
            } else {
                escaped.append(ch)
            }
        }
        escaped.append("\"")
        return escaped
    }

    /// Builds a `{{date ...}}` tag.
    static func dateTag(format: String?, offset: String? = nil, locale: String? = nil) -> String {
        var attrs: [String] = []
        if let format, !format.isEmpty {
            attrs.append("format=\(quoteAttribute(format))")
        }
        if let offset, !offset.isEmpty {
            attrs.append("offset=\(quoteAttribute(offset))")
        }
        if let locale, !locale.isEmpty {
            attrs.append("locale=\(quoteAttribute(locale))")
        }
        if attrs.isEmpty {
            return "{{date}}"
        }
        return "{{date \(attrs.joined(separator: " "))}}"
    }

    /// Builds a `{{clipboard}}` tag.
    static func clipboardTag(trim: Bool = false) -> String {
        if trim {
            return "{{clipboard trim=true}}"
        }
        return "{{clipboard}}"
    }

    /// Builds a `{{cursor}}` tag.
    static var cursorTag: String {
        "{{cursor}}"
    }

    /// Builds a `{{snippet id="..."}}` tag.
    static func snippetTag(id: UUID) -> String {
        "{{snippet id=\(quoteAttribute(id.uuidString))}}"
    }

    /// Builds a `{{field ...}}` tag.
    static func fieldTag(name: String, defaultValue: String? = nil, multiline: Bool = false) -> String {
        var attrs: [String] = ["name=\(quoteAttribute(name))"]
        if let defaultValue, !defaultValue.isEmpty {
            attrs.append("default=\(quoteAttribute(defaultValue))")
        }
        if multiline {
            attrs.append("multiline=true")
        }
        return "{{field \(attrs.joined(separator: " "))}}"
    }

    /// Builds a `{{choice ...}}` tag, escaping `|` in option values as `\|`.
    static func choiceTag(name: String, options: [String], defaultValue: String? = nil) -> String {
        let joinedOptions = options.map { option in
            option
                .replacingOccurrences(of: "\\", with: "\\\\")
                .replacingOccurrences(of: "|", with: "\\|")
        }.joined(separator: "|")
        var attrs: [String] = [
            "name=\(quoteAttribute(name))",
            "options=\(quoteAttribute(joinedOptions))",
        ]
        if let defaultValue, !defaultValue.isEmpty {
            attrs.append("default=\(quoteAttribute(defaultValue))")
        }
        return "{{choice \(attrs.joined(separator: " "))}}"
    }

    /// Splits a pipe-separated options string where `\|` is an escaped pipe.
    static func splitChoiceOptions(_ raw: String) -> [String] {
        var results: [String] = []
        var current = ""
        let chars = Array(raw)
        var i = 0
        while i < chars.count {
            if chars[i] == "\\", i + 1 < chars.count {
                let next = chars[i + 1]
                if next == "|" || next == "\\" {
                    current.append(next)
                    i += 2
                    continue
                }
            }
            if chars[i] == "|" {
                results.append(current)
                current = ""
            } else {
                current.append(chars[i])
            }
            i += 1
        }
        results.append(current)
        return results
    }

    // MARK: - Lightweight template scanner for EspansoExporter

    enum Segment: Equatable {
        case literal(String)
        case tag(name: String, attributes: [String: String], raw: String)
    }

    static func parseSegments(_ template: String) -> [Segment] {
        var segments: [Segment] = []
        var literal = ""
        let chars = Array(template)
        var i = 0

        func flushLiteral() {
            guard !literal.isEmpty else { return }
            segments.append(.literal(literal))
            literal = ""
        }

        while i < chars.count {
            // Check for escaped \{{
            if chars[i] == "\\", i + 2 < chars.count, chars[i + 1] == "{", chars[i + 2] == "{" {
                literal += "{{"
                i += 3
                continue
            }

            // Check for tag start {{
            if chars[i] == "{", i + 1 < chars.count, chars[i + 1] == "{" {
                if let (tagSegment, nextIndex) = tryParseTag(chars, from: i) {
                    flushLiteral()
                    segments.append(tagSegment)
                    i = nextIndex
                    continue
                }
            }

            literal.append(chars[i])
            i += 1
        }

        flushLiteral()
        return segments
    }

    private static func tryParseTag(_ chars: [Character], from start: Int) -> (Segment, Int)? {
        var i = start + 2 // skip {{
        // Scan until unquoted }}
        var inQuotes = false
        var end: Int?
        var scan = i
        while scan < chars.count {
            if inQuotes {
                if chars[scan] == "\\", scan + 1 < chars.count {
                    scan += 2
                    continue
                }
                if chars[scan] == "\"" {
                    inQuotes = false
                }
                scan += 1
            } else {
                if chars[scan] == "\"" {
                    inQuotes = true
                    scan += 1
                } else if chars[scan] == "}", scan + 1 < chars.count, chars[scan + 1] == "}" {
                    end = scan
                    break
                } else {
                    scan += 1
                }
            }
        }

        guard let tagEnd = end else { return nil }
        let raw = String(chars[start...(tagEnd + 1)])

        // Skip leading whitespace
        while i < tagEnd && chars[i].isWhitespace { i += 1 }
        // Parse tag name [a-z][a-z0-9_]*
        let nameStart = i
        while i < tagEnd && (chars[i].isLetter || chars[i].isNumber || chars[i] == "_") {
            i += 1
        }
        guard i > nameStart else { return nil }
        let name = String(chars[nameStart..<i])

        var attrs: [String: String] = [:]
        while i < tagEnd {
            while i < tagEnd && chars[i].isWhitespace { i += 1 }
            if i >= tagEnd { break }

            let keyStart = i
            while i < tagEnd && (chars[i].isLetter || chars[i].isNumber || chars[i] == "_") {
                i += 1
            }
            guard i > keyStart else { return nil }
            let key = String(chars[keyStart..<i])

            while i < tagEnd && chars[i].isWhitespace { i += 1 }
            guard i < tagEnd, chars[i] == "=" else { return nil }
            i += 1
            while i < tagEnd && chars[i].isWhitespace { i += 1 }
            guard i < tagEnd else { return nil }

            if chars[i] == "\"" {
                i += 1
                var value = ""
                var closed = false
                while i < tagEnd {
                    if chars[i] == "\\", i + 1 < tagEnd {
                        let escaped = chars[i + 1]
                        if escaped == "\"" || escaped == "\\" {
                            value.append(escaped)
                            i += 2
                            continue
                        }
                    }
                    if chars[i] == "\"" {
                        i += 1
                        closed = true
                        break
                    }
                    value.append(chars[i])
                    i += 1
                }
                guard closed else { return nil }
                attrs[key] = value
            } else {
                let valStart = i
                while i < tagEnd && !chars[i].isWhitespace {
                    i += 1
                }
                attrs[key] = String(chars[valStart..<i])
            }
        }

        return (.tag(name: name, attributes: attrs, raw: raw), tagEnd + 2)
    }
}
