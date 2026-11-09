import Foundation

/// A small YAML tree: scalar values stay strings, interpreted by each Espanso key.
struct YAMLNode: Equatable, Sendable {
    indirect enum Kind: Equatable, Sendable {
        case scalar(String), sequence([YAMLNode]), mapping([(String,YAMLNode)])
        static func == (lhs: Self, rhs: Self) -> Bool {
            switch (lhs,rhs) {
            case (.scalar(let a),.scalar(let b)): a.isExactly(b)
            case (.sequence(let a),.sequence(let b)): a == b
            case (.mapping(let a),.mapping(let b)): a.count == b.count && zip(a,b).allSatisfy { $0.0.isExactly($1.0) && $0.1 == $1.1 }
            default: false
            }
        }
    }
    var kind: Kind
    var line: Int
    var string: String? { if case .scalar(let s) = kind { s } else { nil } }
    var sequence: [YAMLNode] { if case .sequence(let s) = kind { s } else { [] } }
    var mapping: [(String,YAMLNode)]? { if case .mapping(let m) = kind { m } else { nil } }
    subscript(_ key: String) -> YAMLNode? { mapping?.first { $0.0.isExactly(key) }?.1 }
    func str(_ key: String) -> String? { self[key]?.string }
    func bool(_ key: String) -> Bool? { switch str(key)?.lowercased() { case "true","yes": true; case "false","no": false; default: nil } }
}

/// YAML subset used by Espanso: blocks, flows, quoting, comments and block scalars.
/// No Foundation/AppKit parser or third-party package is involved.
enum YAML {
    struct ParseError: Error, CustomStringConvertible { var description: String }
    struct Line {
        var raw: String
        var number: Int
        var indent: Int { raw.unicodeScalars.prefix { $0 == " " }.count }
        var content: String { String(raw.unicodeScalars.dropFirst(indent)) }
    }
    /// yaml-rust2 structural blanks are space and tab only. NBSP and U+200B are content.
    static func chars(_ s: String) -> [Character] { RustText.chars(s) }
    static func isBlank(_ c: Character) -> Bool { c == " " || c == "\t" }
    static func trimBlank(_ s: String) -> String {
        var scalars = Array(s.unicodeScalars)
        while scalars.first == " " || scalars.first == "\t" { scalars.removeFirst() }
        while scalars.last == " " || scalars.last == "\t" { scalars.removeLast() }
        return String(String.UnicodeScalarView(scalars))
    }
    static func parse(_ text: String) throws -> YAMLNode? {
        var scalars: [Unicode.Scalar] = []
        let source = Array(text.unicodeScalars)
        var n = 0
        while n < source.count {
            if source[n] == "\r" {
                scalars.append("\n")
                if n + 1 < source.count && source[n + 1] == "\n" { n += 1 }
            } else { scalars.append(source[n]) }
            n += 1
        }
        let normalized = String(String.UnicodeScalarView(scalars))
        var lines = RustText.split(normalized, separator: "\n", omittingEmptySubsequences: false)
        if scalars.last == "\n" { lines.removeLast() }
        var reader = Reader(lines:lines.enumerated().map { Line(raw:$0.element,number:$0.offset+1) })
        reader.skip()
        if reader.i < reader.lines.count && reader.lines[reader.i].content == "---" { reader.i += 1; reader.skip() }
        guard reader.i < reader.lines.count else { return nil }
        return try reader.block(reader.lines[reader.i].indent)
    }
    static func split(_ s: String, separator: Character, mapping: Bool = false) -> [String] {
        let c = chars(s); var quote: Character?, depth = 0, parts: [String] = [], start = 0, i = 0
        while i < c.count {
            let ch = c[i]
            if let q = quote {
                if ch == "\\" && q == "\"" { i += 2; continue }
                if ch == q { if q == "'" && i+1 < c.count && c[i+1] == "'" { i += 2; continue }; quote = nil }
            } else if ch == "'" || ch == "\"" { quote = ch }
            else if ch == "[" || ch == "{" { depth += 1 }
            else if ch == "]" || ch == "}" { depth -= 1 }
            else if ch == separator && depth == 0 && (!mapping || i+1 == c.count || isBlank(c[i+1])) {
                parts.append(String(c[start..<i])); start = i+1
                if mapping { parts.append(String(c[start...])); return parts }
            }; i += 1
        }; parts.append(String(c[start...])); return parts
    }
    static func uncomment(_ s: String) -> String {
        let c = chars(s); var q: Character?, i = 0
        while i < c.count {
            if let quote = q { if c[i] == "\\" && quote == "\"" { i += 2; continue }; if c[i] == quote { q = nil } }
            else if c[i] == "\"" || c[i] == "'" { q = c[i] }
            else if c[i] == "#" && (i == 0 || isBlank(c[i-1])) { return trimBlank(String(c[..<i])) }; i += 1
        }; return trimBlank(s)
    }
    struct Reader {
        var lines: [Line]; var i = 0; var anchors: [String:YAMLNode] = [:]
        mutating func skip() { while i < lines.count && YAML.uncomment(lines[i].content).isEmpty { i += 1 } }
        mutating func block(_ indent: Int) throws -> YAMLNode {
            skip(); guard i < lines.count else { return YAMLNode(kind:.scalar(""),line:lines.last?.number ?? 1) }
            let line = lines[i], c = YAML.uncomment(line.content)
            if c == "-" || RustText.hasPrefix(c, "- ") {
                var seq: [YAMLNode] = []
                while i < lines.count {
                    skip(); guard i < lines.count, lines[i].indent == indent else { break }
                    let l = lines[i], value = YAML.uncomment(l.content)
                    guard value == "-" || RustText.hasPrefix(value, "- ") else { break }
                    let rest = RustText.dropFirst(value, value == "-" ? 1 : 2); i += 1
                    if YAML.split(rest,separator:":",mapping:true).count == 2 {
                        var entries: [(String,YAMLNode)] = []
                        try pair(rest,indent:indent+2,line:l.number,entries:&entries)
                        while true {
                            skip(); guard i < lines.count, lines[i].indent > indent else { break }
                            let next = lines[i]; i += 1
                            try pair(YAML.uncomment(next.content),indent:next.indent,line:next.number,entries:&entries)
                        }
                        seq.append(YAMLNode(kind:.mapping(entries),line:l.number))
                    } else { seq.append(try self.value(rest,parent:indent,line:l.number)) }
                }
                return YAMLNode(kind:.sequence(seq),line:line.number)
            }
            if YAML.split(c,separator:":",mapping:true).count == 2 {
                var entries: [(String,YAMLNode)] = []
                while i < lines.count {
                    skip(); guard i < lines.count, lines[i].indent == indent else { break }
                    let l = lines[i], c = YAML.uncomment(l.content)
                    if c == "---" || c == "..." || YAML.split(c,separator:":",mapping:true).count != 2 { break }
                    i += 1; try pair(c,indent:indent,line:l.number,entries:&entries)
                }; return YAMLNode(kind:.mapping(entries),line:line.number)
            }
            i += 1; return try scalar(c,line:line.number)
        }
        mutating func pair(_ source: String, indent: Int, line: Int, entries: inout [(String,YAMLNode)]) throws {
            let parts = YAML.split(source,separator:":",mapping:true)
            guard parts.count == 2 else { throw ParseError(description:"expected mapping at line \(line)") }
            let key = try scalar(YAML.trimBlank(parts[0]),line:line).string ?? ""
            entries.append((key,try value(YAML.trimBlank(parts[1]),parent:indent,line:line)))
        }
        mutating func value(_ source: String, parent: Int, line: Int) throws -> YAMLNode {
            var s = source, anchor: String?
            if RustText.hasPrefix(s, "&") {
                let name = YAML.chars(RustText.dropFirst(s, 1)).prefix { !YAML.isBlank($0) }
                anchor = String(name)
                s = YAML.trimBlank(RustText.dropFirst(s, name.count + 1))
            }
            let node: YAMLNode
            if s.isEmpty {
                skip()
                if i < lines.count && (lines[i].indent > parent || (lines[i].indent == parent && RustText.hasPrefix(lines[i].content, "-"))) { node = try block(lines[i].indent) }
                else { node = YAMLNode(kind:.scalar(""),line:line) }
            } else if let head = s.unicodeScalars.first, head == "|" || head == ">" {
                let folded = head == ">", strip = s.unicodeScalars.contains("-"), keep = s.unicodeScalars.contains("+")
                var end = i
                while end < lines.count && (YAML.trimBlank(lines[end].raw).isEmpty || lines[end].indent > parent) { end += 1 }
                let nonblank = lines[i..<end].filter { !YAML.trimBlank($0.raw).isEmpty }
                let explicit = s.compactMap { $0.wholeNumberValue }.first
                let indent = explicit.map { parent+$0 } ?? nonblank.first?.indent ?? parent+1
                var body = "", seenContent = false, previousIndented = false
                for n in i..<end {
                    let t = String(lines[n].raw.unicodeScalars.dropFirst(min(indent, lines[n].raw.unicodeScalars.count)))
                    let next = n+1 < end ? String(lines[n+1].raw.unicodeScalars.dropFirst(min(indent, lines[n+1].raw.unicodeScalars.count))) : ""
                    body += t
                    if !folded { body += "\n" }
                    else if t.isEmpty {
                        if next.isEmpty || !seenContent || previousIndented { body += "\n" }
                    } else {
                        body += !next.isEmpty && !RustText.hasPrefix(t, " ") && !RustText.hasPrefix(next, " ") ? " " : "\n"
                        seenContent = true; previousIndented = RustText.hasPrefix(t, " ")
                    }
                }
                i = end
                if !keep { while body.last == "\n" { body.removeLast() }; if !strip && !body.isEmpty { body += "\n" } }
                node = YAMLNode(kind:.scalar(body),line:line)
            } else {
                let quote = s.unicodeScalars.first == "'" || s.unicodeScalars.first == "\"" ? s.unicodeScalars.first.map { Character($0) } : nil
                let flow = s.unicodeScalars.first == "[" || s.unicodeScalars.first == "{"
                var blanks = 0
                while i < lines.count {
                    if quote != nil && quotedComplete(s) { break }
                    if flow && flowComplete(s) { break }
                    if YAML.trimBlank(lines[i].content).isEmpty { blanks += 1; i += 1; continue }
                    if !flow && lines[i].indent <= parent { break }
                    let continuation = YAML.uncomment(lines[i].content)
                    if !flow && quote == nil && YAML.split(continuation,separator:":",mapping:true).count == 2 { throw ParseError(description:"mapping in plain scalar at line \(lines[i].number)") }
                    let escaped = quote == "\"" && s.unicodeScalars.reversed().prefix { $0 == "\\" }.count % 2 == 1
                    if escaped { s = String(s.unicodeScalars.dropLast()) }
                    s += escaped ? "" : blanks > 0 ? String(repeating:"\n",count:blanks) : " "
                    s += continuation; i += 1; blanks = 0
                }
                node = try scalar(s,line:line)
            }
            if let anchor { anchors[anchor] = node }; return node
        }
        func quotedComplete(_ s: String) -> Bool {
            let chars = YAML.chars(s); guard let q = chars.first else { return false }; var j = 1
            while j < chars.count {
                if q == "\"" && chars[j] == "\\" { j += 2; continue }
                if chars[j] == q { if q == "'" && j+1 < chars.count && chars[j+1] == "'" { j += 2; continue }; return true }; j += 1
            }; return false
        }
        func flowComplete(_ s: String) -> Bool {
            let c = YAML.chars(s); var depth = 0, q: Character?, j = 0
            while j < c.count {
                if let quote = q { if quote == "\"" && c[j] == "\\" { j += 2; continue }; if c[j] == quote { q = nil } }
                else if c[j] == "'" || c[j] == "\"" { q = c[j] }
                else if c[j] == "[" || c[j] == "{" { depth += 1 }
                else if c[j] == "]" || c[j] == "}" { depth -= 1 }
                j += 1
            }; return depth == 0 && q == nil
        }
        mutating func scalar(_ source: String, line: Int) throws -> YAMLNode {
            let s = YAML.uncomment(source), chars = YAML.chars(s)
            if RustText.hasPrefix(s, "*") { return anchors[RustText.dropFirst(s, 1)] ?? YAMLNode(kind:.scalar(""),line:line) }
            if RustText.hasPrefix(s, "[") || RustText.hasPrefix(s, "{") {
                let seq = RustText.hasPrefix(s, "[")
                guard chars.last == (seq ? "]" : "}") else { throw ParseError(description:"unclosed flow collection at line \(line)") }
                let inner = String(chars.dropFirst().dropLast()), parts = YAML.split(inner,separator:",").filter { !YAML.trimBlank($0).isEmpty }
                if seq { return YAMLNode(kind:.sequence(try parts.map { try scalar(YAML.trimBlank($0),line:line) }),line:line) }
                var map: [(String,YAMLNode)] = []
                for p in parts {
                    let kv = YAML.split(p,separator:":")
                    guard kv.count >= 2 else { throw ParseError(description:"expected flow mapping at line \(line)") }
                    let k = try scalar(YAML.trimBlank(kv[0]),line:line).string ?? ""
                    map.append((k,try scalar(YAML.trimBlank(kv.dropFirst().joined(separator:":")),line:line)))
                }; return YAMLNode(kind:.mapping(map),line:line)
            }
            if let q = chars.first, q == "'" || q == "\"" {
                guard chars.count >= 2 && chars.last == q else { throw ParseError(description:"unclosed quoted scalar at line \(line)") }
                var out = "", k = 1
                while k < chars.count-1 {
                    if q == "'" && chars[k] == "'" && k+1 < chars.count-1 && chars[k+1] == "'" { out += "'"; k += 2; continue }
                    if q == "\"" && chars[k] == "\\" {
                        k += 1; guard k < chars.count-1 else { throw ParseError(description:"bad escape at line \(line)") }
                        let escapes: [Character:String] = ["0":"\0","a":"\u{7}","b":"\u{8}","t":"\t","n":"\n","v":"\u{b}","f":"\u{c}","r":"\r","e":"\u{1b}"," ":" ","\"":"\"","/":"/","\\":"\\","N":"\u{85}","_":"\u{a0}","L":"\u{2028}","P":"\u{2029}"]
                        if let e = escapes[chars[k]] { out += e }
                        else if let n = ["x":2,"u":4,"U":8][String(chars[k])], k+n < chars.count-1, let v = UInt32(String(chars[(k+1)...(k+n)]),radix:16), let u = UnicodeScalar(v) { out.unicodeScalars.append(u); k += n }
                        else { throw ParseError(description:"bad escape at line \(line)") }
                    } else { out.append(chars[k]) }; k += 1
                }; return YAMLNode(kind:.scalar(out),line:line)
            }
            return YAMLNode(kind:.scalar(s.isExactly("~") ? "" : s),line:line)
        }
    }
}
