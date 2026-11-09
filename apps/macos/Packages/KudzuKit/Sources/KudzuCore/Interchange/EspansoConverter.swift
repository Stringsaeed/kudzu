import Foundation

struct EspansoConverter {
    var vars: ExactMap<YAMLNode>; var triggers: ExactMap<UUID>; var file: String; var index: Int; var trigger: String?; var line: Int
    var warnings: [ImportWarning] = []
    mutating func warn(_ message: String, _ line: Int?) { warnings.append(ImportWarning(file:file,line:line ?? self.line,matchIndex:index,trigger:trigger,message:message)) }
    /// `trim_matches([' ', '\t'])`: U+200B and other Unicode spaces stay in the name.
    func trimField(_ name: String) -> String {
        var scalars = Array(name.unicodeScalars)
        while scalars.first == " " || scalars.first == "\t" { scalars.removeFirst() }
        while scalars.last == " " || scalars.last == "\t" { scalars.removeLast() }
        return String(String.UnicodeScalarView(scalars))
    }
    mutating func convert(_ raw: String, fields: YAMLNode?, line: Int) -> String {
        let chars = RustText.chars(raw); var out = "", i = 0
        func at(_ p: String) -> Bool { let a = Array(p); return i+a.count <= chars.count && Array(chars[i..<(i+a.count)]) == a }
        func close(_ from: Int, _ ch: Character) -> Int? { guard from+1 < chars.count else { return nil }; return (from..<(chars.count-1)).first { chars[$0] == ch && chars[$0+1] == ch } }
        while i < chars.count {
            if at("$|$") { out += ImportTags.cursor(); i += 3; continue }
            if at("\\{\\{") { out += "\\{{"; i += 4; continue }
            if fields == nil && at("\\{{") { out += "\\{{"; i += 3; continue }
            if let fields, at("[["), let end = close(i+2,"]") {
                let name = trimField(String(chars[(i+2)..<end]))
                if !name.isEmpty { out += field(name,fields[name],line:line); i = end+2; continue }
            }
            if at("{{") {
                if let end = close(i+2,"}") {
                    let name = trimField(String(chars[(i+2)..<end]))
                    out += resolve(name,line:line); i = end+2
                } else { out += "\\{{"; i += 2 }; continue
            }
            out += chars[i] == "\\" ? "\\\\" : String(chars[i]); i += 1
        }; return out
    }
    mutating func resolve(_ reference: String, line: Int) -> String {
        let literal = "\\{{\(reference)}}", parts = RustText.split(reference, separator: ".", maxSplits: 1, omittingEmptySubsequences: false)
        if parts.count == 2, let form = vars[parts[0]], form.str("type") == "form" { return field(parts[1],form["params"]?["fields"]?[parts[1]],line:form.line) }
        guard let v = vars[reference] else { warn("Undefined variable '{{\(reference)}}' kept as literal text.",line); return literal }
        let params = v["params"], name = v.str("name") ?? reference, line = v.line
        switch v.str("type") ?? "" {
        case "date":
            let converted = Strftime.toICU(params?.str("format") ?? "%Y-%m-%d")
            if !converted.unsupportedSpecifiers.isEmpty { warn("Unsupported strftime specifier(s) \(converted.unsupportedSpecifiers.joined(separator:", ")) in date variable '\(name)' kept as literal.",line) }
            let offset = params?.str("offset").flatMap { Int64(RustText.trim($0)) }.flatMap(Strftime.secondsToKudzuOffset)
            if params?["tz"] != nil { warn("Date variable '\(name)' specifies 'tz' which is not supported in v1 and was ignored.",line) }
            return ImportTags.date(converted.icuFormat,offset,params?.str("locale"))
        case "clipboard": return ImportTags.clipboard()
        case "match":
            guard let t = params?.str("trigger"), !t.isEmpty else { warn("Nested match variable '\(name)' is missing 'params.trigger'; kept as literal text.",line); return literal }
            if let id = triggers[t] { return ImportTags.snippet(id) }
            warn("Unresolved nested match trigger '\(t)' for variable '\(name)'; kept as literal text.",line); return literal
        case "form":
            if let layout = params?.str("layout") { return convert(layout,fields:params?["fields"] ?? YAMLNode(kind:.mapping([]),line:line),line:line) }
            warn("Form variable '\(name)' referenced directly without a field name; kept as literal text.",line); return literal
        default: warn("Unsupported variable type '\(v.str("type") ?? "")' for variable '\(name)'; kept as literal text.",line); return literal
        }
    }
    mutating func field(_ name: String, _ config: YAMLNode?, line: Int) -> String {
        guard let config, config.mapping != nil else { return ImportTags.field(name) }
        let kind = config.str("type") ?? "text", value = config.str("default"), multi = config.bool("multiline") ?? false
        if ["choice","list"].contains(kind.lowercased()) {
            var values = config["values"]?.sequence.compactMap(\.string) ?? []
            if let string = config["values"]?.string {
                let trim = config.bool("trim_string_values") ?? true
                let breaks: Set<Unicode.Scalar> = ["\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}"]
                values = RustText.split(string, where: { breaks.contains($0) }).map { trim ? trimField($0) : $0 }.filter { !$0.isEmpty }
            }; return ImportTags.choice(name,values,value)
        }
        if !["text",""].contains(kind.lowercased()) { warn("Unknown form field control type '\(kind)' for field '\(name)'; imported as text field.",line) }
        return ImportTags.field(name,value,multi)
    }
}
