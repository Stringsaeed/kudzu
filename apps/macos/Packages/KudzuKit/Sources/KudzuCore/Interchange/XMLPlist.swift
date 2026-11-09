import Foundation

/// TextExpander's hand-reader rules supplement Foundation's property-list validation:
/// dates retain text, CDATA/whitespace survive, unknown entities remain literal, depth is bounded.
indirect enum XMLPlist {
    case dict([(String,XMLPlist)]), array([XMLPlist]), string(String), int(Int64), real(Double), bool(Bool), date(String), data(String)
    subscript(_ key: String) -> XMLPlist? { if case .dict(let entries) = self { entries.first { $0.0.isExactly(key) }?.1 } else { nil } }
    var text: String? { if case .string(let s) = self { s } else { nil } }
    var items: [XMLPlist]? { if case .array(let a) = self { a } else { nil } }
    var integer: Int64? { switch self { case .int(let n): n; case .real(let n): n.isFinite ? Int64(clamping:NSNumber(value:n).int64Value) : nil; case .bool(let b): b ? 1 : 0; default: nil } }
    var dateText: String? { if case .date(let s) = self { s } else { nil } }
    enum Token { case open(String), close(String), empty(String), text(String) }
    struct Failure: Error, CustomStringConvertible { var description: String }
    static func parse(_ xml: String) throws -> XMLPlist {
        // Foundation recognizes the standard plist envelope; the hand reader below retains
        // legacy TextExpander details that PropertyListSerialization normalizes away.
        _ = try? PropertyListSerialization.propertyList(from:Data(xml.utf8),options:[],format:nil)
        let tokens = try tokenize(xml); var i = 0
        func skip() { while i < tokens.count { if case .text(let s) = tokens[i], RustText.trim(s).isEmpty { i += 1 } else { break } } }
        func readText(_ name: String) throws -> String {
            var out = ""
            while i < tokens.count {
                switch tokens[i] {
                case .text(let t): out += t; i += 1
                case .close(let n) where n == name: i += 1; return out
                default: throw Failure(description:"<\(name)> was never closed")
                }
            }; throw Failure(description:"<\(name)> was never closed")
        }
        func value(_ depth: Int) throws -> XMLPlist {
            if depth > 64 { throw Failure(description:"the property list is nested too deeply") }; skip()
            guard i < tokens.count else { throw Failure(description:"the property list ended early") }
            let token = tokens[i]; i += 1
            switch token {
            case .empty(let name):
                switch name { case "true": return .bool(true); case "false": return .bool(false); case "string": return .string(""); case "dict": return .dict([]); case "array": return .array([]); case "data": return .data(""); default: throw Failure(description:"unexpected <\(name)/>") }
            case .open(let name):
                switch name {
                case "dict":
                    var entries: [(String,XMLPlist)] = []
                    while true {
                        skip(); guard i < tokens.count else { throw Failure(description:"expected <key> inside <dict>") }
                        if case .close("dict") = tokens[i] { i += 1; return .dict(entries) }
                        let key: String
                        if case .open("key") = tokens[i] { i += 1; key = try readText("key") }
                        else if case .empty("key") = tokens[i] { i += 1; key = "" }
                        else { throw Failure(description:"expected <key> inside <dict>") }
                        entries.append((key,try value(depth+1)))
                    }
                case "array":
                    var items: [XMLPlist] = []
                    while true { skip(); guard i < tokens.count else { throw Failure(description:"the property list ended inside an <array>") }; if case .close("array") = tokens[i] { i += 1; return .array(items) }; items.append(try value(depth+1)) }
                case "string": return .string(try readText(name))
                case "date": return .date(RustText.trim(try readText(name)))
                case "data": return .data(try readText(name))
                case "integer": let s = RustText.trim(try readText(name)); guard let n = Int64(s) else { throw Failure(description:"bad integer '\(s)'") }; return .int(n)
                case "real": let s = RustText.trim(try readText(name)); guard let n = Double(s) else { throw Failure(description:"bad number '\(s)'") }; return .real(n)
                case "true","false": _ = try readText(name); return .bool(name == "true")
                default: throw Failure(description:"unexpected <\(name)>")
                }
            case .close(let name): throw Failure(description:"unexpected </\(name)>")
            case .text: throw Failure(description:"unexpected text")
            }
        }
        skip()
        while i < tokens.count { if case .open("plist") = tokens[i] { i += 1 } else if case .empty("plist") = tokens[i] { i += 1 } else { break } }
        return try value(0)
    }
    static func normalize(_ s: String) -> String { s.replacingOccurrences(of:"\r\n",with:"\n").replacingOccurrences(of:"\r",with:"\n") }
    static func starts(_ s: Substring, _ prefix: String) -> Bool { RustText.hasPrefix(String(s), prefix) }
    static func dropScalars(_ s: Substring, _ n: Int) -> Substring { Substring(String(s.unicodeScalars.dropFirst(n))) }
    static func tokenize(_ xml: String) throws -> [Token] {
        var rest = xml[...], out: [Token] = []
        while !rest.isEmpty {
            if starts(rest, "<![CDATA[") || starts(rest, "<!--") || starts(rest, "<?") {
                let cdata = starts(rest, "<![CDATA["), comment = starts(rest, "<!--"), opening = cdata ? 9 : comment ? 4 : 2, closing = cdata ? "]]>" : comment ? "-->" : "?>"
                let after = dropScalars(rest, opening); guard let end = after.range(of:closing) else { throw Failure(description:cdata ? "unterminated CDATA section" : comment ? "unterminated comment" : "unterminated declaration") }
                if cdata { out.append(.text(normalize(String(after[..<end.lowerBound])))) }; rest = after[end.upperBound...]
            } else if starts(rest, "<!") {
                var depth = 0, end: String.Index?
                for i in rest.indices { if rest[i] == "[" { depth += 1 }; if rest[i] == "]" { depth -= 1 }; if rest[i] == ">" && depth <= 0 { end = i; break } }
                guard let end else { throw Failure(description:"unterminated declaration") }; rest = rest[rest.index(after:end)...]
            } else if rest.unicodeScalars.first == "<" {
                let scalars = Array(rest.unicodeScalars)
                guard let gt = scalars.firstIndex(of: ">") else { throw Failure(description:"unterminated tag") }
                let body = RustText.trim(String(String.UnicodeScalarView(scalars[1..<gt])))
                if RustText.hasPrefix(body, "/") { out.append(.close(RustText.trim(RustText.dropFirst(body, 1)))) }
                else {
                    var nameScalars = Array(body.unicodeScalars)
                    let empty = nameScalars.last == "/"
                    if empty { nameScalars.removeLast() }
                    let name = RustText.split(String(String.UnicodeScalarView(nameScalars)), where: { $0 == " " || $0 == "\t" || $0 == "\n" || $0 == "\r" }).first ?? ""
                    out.append(empty ? .empty(name) : .open(name))
                }
                rest = dropScalars(rest, gt + 1)
            } else {
                let scalars = Array(rest.unicodeScalars)
                let lt = scalars.firstIndex(of: "<") ?? scalars.count
                out.append(.text(entities(normalize(String(String.UnicodeScalarView(scalars[..<lt]))))))
                rest = dropScalars(rest, lt)
            }
        }; return out
    }
    static func entities(_ source: String) -> String {
        var rest = source[...], out = ""
        while let amp = rest.firstIndex(of:"&") {
            out += rest[..<amp]; let after = rest[rest.index(after:amp)...]
            if let semi = after.firstIndex(of:";"), after[..<semi].utf8.count <= 10 {
                let name = String(after[..<semi]); var decoded = ["amp":"&","lt":"<","gt":">","quot":"\"","apos":"'"][name]
                if decoded == nil {
                    let n = RustText.hasPrefix(name, "#x") || RustText.hasPrefix(name, "#X") ? UInt32(RustText.dropFirst(name, 2), radix: 16) : RustText.hasPrefix(name, "#") ? UInt32(RustText.dropFirst(name, 1)) : nil
                    if let n, n != 0, let scalar = UnicodeScalar(n) { decoded = String(scalar) }
                }
                if let decoded { out += decoded; rest = after[after.index(after:semi)...]; continue }
            }; out += "&"; rest = after
        }; out += rest; return out
    }
}
