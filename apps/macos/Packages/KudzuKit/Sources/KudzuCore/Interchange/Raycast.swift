// Ported from kudzu-core/src/interchange/raycast.rs
// Raycast snippet export (JSON) → Kudzu library.
//
// Raycast's "Export Snippets" writes a JSON array of `{ "name", "text", "keyword" }` objects.
// Placeholders (`{clipboard}`, `{date …}`, `{argument …}`, …) become Kudzu tags; the ones Kudzu
// can't run stay as literal text with a warning. See the Raycast mapping table in
// docs/TEMPLATE_SYNTAX.md.

import Foundation

/// Raycast snippet export JSON and placeholder conversion.
public enum Raycast {
    public static let GROUP_NAME = "Raycast"
    static let notRaycast = "This isn't a Raycast snippet export. In Raycast, run “Export Snippets” and choose the JSON file it saves."
    /// Reads a Raycast snippet export from `path`.
    public static func importFile(_ path: URL) throws -> ImportResult { try importJSON(String(contentsOf:path,encoding:.utf8),fileName:path.lastPathComponent) }
    public static func importJSON(_ json: String, fileName: String) throws -> ImportResult {
        let source = String(json.drop(while: { $0 == "\u{feff}" }))
        guard let items = Sync.parsed(source) as? [Any] else { throw InterchangeError.invalidSource(notRaycast) }
        if items.isEmpty { return ImportResult() }
        struct Entry { var id = UUID(); var index: Int; var name: String; var text: String; var keyword: String? }
        var entries: [Entry] = [], warnings: [ImportWarning] = []
        for (index,item) in items.enumerated() {
            guard let obj = item as? [String:Any] else { warnings.append(ImportWarning(file:fileName,matchIndex:index,message:"Entry \(index) isn't a snippet and was skipped.")); continue }
            let keyword = (obj["keyword"] as? String).map(RustText.trim)?.nilIfEmpty
            guard let text = obj["text"] as? String, !text.isEmpty else { warnings.append(ImportWarning(file:fileName,matchIndex:index,trigger:keyword,message:"Snippet has no text and was skipped.")); continue }
            let name = (obj["name"] as? String).map(RustText.trim)?.nilIfEmpty ?? keyword ?? firstLine(text)
            entries.append(Entry(index:index,name:name,text:text,keyword:keyword))
        }
        if entries.isEmpty { throw InterchangeError.invalidSource(notRaycast) }
        var byName = ExactMap<UUID>(); for e in entries where byName[e.name] == nil { byName[e.name] = e.id }
        let group = Group(name:GROUP_NAME), now = Date(timeIntervalSince1970:floor(Date().timeIntervalSince1970))
        var snippets: [Snippet] = []
        for e in entries {
            var problems: [String] = []; let content = convert(e.text,snippets:byName,problems:&problems)
            for message in problems { warnings.append(ImportWarning(file:fileName,matchIndex:e.index,trigger:e.keyword,message:message)) }
            if e.keyword == nil { warnings.append(ImportWarning(file:fileName,matchIndex:e.index,message:"'\(e.name)' has no keyword, so it was imported turned off. Add a trigger to use it.")) }
            snippets.append(Snippet(id:e.id,name:e.name,triggers:e.keyword.map { [$0] } ?? [],content:content,groupID:group.id,caseMode:.exact,isEnabled:e.keyword != nil,createdAt:now,updatedAt:now))
        }; return ImportResult(library:Library(groups:[group],snippets:snippets),warnings:warnings)
    }
    static func firstLine(_ text: String) -> String { Clipboard.excerpt(RustText.trim(RustText.lines(text).first ?? ""),limit:40).nilIfEmpty ?? "Untitled" }
    struct Placeholder { var name: String; var attrs: [(String,String)]; var modifiers: [String]; func attr(_ key: String) -> String? { attrs.first { $0.0 == key }?.1 } }
    static let names = ["clipboard","cursor","date","time","datetime","day","selection","uuid","snippet","argument"]
    static func span(_ c: [Character], _ open: Int) -> (Int,String)? {
        guard open < c.count && c[open] == "{" else { return nil }; var quoted = false, i = open+1
        while i < c.count {
            if c[i] == "\\" && quoted { i += 2; continue }
            if c[i] == "\"" { quoted.toggle() }
            if c[i] == "\n" && !quoted { return nil }
            if c[i] == "}" && !quoted {
                let inner = String(c[(open+1)..<i]), name = String(inner.prefix { $0.isASCII && $0.isLowercase })
                let rest = inner.dropFirst(name.count)
                return names.contains(name) && (rest.first == nil || rest.first?.isWhitespace == true || rest.first == "|") ? (i,name) : nil
            }; i += 1
        }; return nil
    }
    static func placeholder(_ name: String, _ rest: String) -> Placeholder? {
        let c = rest.unicodeScalars.map { Character(String($0)) }; var i = 0, attrs: [(String,String)] = [], modifiers: [String] = []
        while i < c.count {
            while i < c.count && c[i].isWhitespace { i += 1 }; if i == c.count { break }
            if c[i] == "|" {
                var pieces: [[Character]] = [[]]
                for ch in c[(i+1)...] { if ch == "|" { pieces.append([]) } else { pieces[pieces.count-1].append(ch) } }
                modifiers = pieces.map { RustText.trim(String($0)) }.filter { !$0.isEmpty }
                break
            }
            let start = i; while i < c.count && ((c[i].isASCII && (c[i].isLetter || c[i].isNumber)) || c[i] == "_" || c[i] == "-") { i += 1 }
            guard i > start && i < c.count && c[i] == "=" else { return nil }
            let key = String(c[start..<i]); i += 1; var value = ""
            if i < c.count && c[i] == "\"" {
                i += 1; var closed = false
                while i < c.count {
                    if c[i] == "\\" && i+1 < c.count && ["\"","\\"].contains(c[i+1]) { value.append(c[i+1]); i += 2 }
                    else if c[i] == "\"" { i += 1; closed = true; break }
                    else { value.append(c[i]); i += 1 }
                }; if !closed { return nil }
            } else { while i < c.count && !c[i].isWhitespace && c[i] != "|" { value.append(c[i]); i += 1 } }
            attrs.append((key,value))
        }; return Placeholder(name:name,attrs:attrs,modifiers:modifiers)
    }
    static func convert(_ raw: String, snippets: ExactMap<UUID> = .init(), problems: inout [String]) -> String {
        let c = raw.unicodeScalars.map { Character(String($0)) }; var out = TemplateOut(), i = 0, unnamed = 0
        while i < c.count {
            if c[i] == "\\" && i+1 < c.count && c[i+1] == "{" { out.text(span(c,i+1) != nil ? "{" : "\\{"); i += 2; continue }
            guard let (end,name) = span(c,i) else { out.pushChar(c[i]); i += 1; continue }
            let source = String(c[i...end]), rest = String(c[(i+1+name.count)..<end])
            if let p = placeholder(name,rest) { if let tag = tag(p,snippets,&unnamed,&problems) { out.tag(tag) } else { out.text(source) } }
            else { problems.append("Couldn't read the placeholder \(source); kept as text."); out.text(source) }; i = end+1
        }; return out.finish()
    }
    static func tag(_ p: Placeholder, _ snippets: ExactMap<UUID>, _ unnamed: inout Int, _ problems: inout [String]) -> String? {
        let label = "{\(p.name)}"; var modifiersUsed = !p.modifiers.isEmpty; let tag: String
        switch p.name {
        case "cursor": tag = ImportTags.cursor()
        case "clipboard":
            if let offset = p.attr("offset"), !RustText.trim(offset).isExactly("0") { problems.append("\(label) with offset=\(offset) reads clipboard history, which isn't supported; kept as text."); return nil }
            modifiersUsed = p.modifiers.contains { !$0.isExactly("trim") }; tag = ImportTags.clipboard(p.modifiers.contains { $0.isExactly("trim") })
        case "date","time","datetime","day":
            let def = ["date":"d MMM yyyy","time":"HH:mm","datetime":"d MMM yyyy 'at' HH:mm","day":"EEEE"][p.name] ?? ""
            var offset: String?
            if let raw = p.attr("offset"), !RustText.trim(raw).isEmpty { offset = convertOffset(raw); if offset == nil { problems.append("Date offset \"\(raw)\" in \(label) isn't supported and was ignored.") } }
            tag = ImportTags.date(p.attr("format")?.nilIfEmpty ?? def,offset,p.attr("locale").map { raw in
                var out = ""
                for scalar in RustText.trim(raw).unicodeScalars { out.unicodeScalars.append(scalar == "-" ? "_" : scalar) }
                return out
            })
        case "snippet":
            guard let name = p.attr("name"), let id = snippets[name] else { problems.append(p.attr("name").map { "\(label) refers to \"\($0)\", which isn't in this export; kept as text." } ?? "\(label) has no name; kept as text."); return nil }; tag = ImportTags.snippet(id)
        case "argument":
            let name: String
            if let n = p.attr("name"), !n.isEmpty { name = n } else { unnamed += 1; name = "argument \(unnamed)" }
            let options = p.attr("options").map { RustText.split($0, separator: ",").map(RustText.trim).filter { !$0.isEmpty } } ?? []
            tag = options.isEmpty ? ImportTags.field(name,p.attr("default")) : ImportTags.choice(name,options,p.attr("default"))
        default: problems.append("\(label) has no Kudzu equivalent; kept as text."); return nil
        }
        if modifiersUsed { problems.append("Modifiers on \(label) (| \(p.modifiers.joined(separator:" | "))) aren't supported and were ignored.") }; return tag
    }
    static func convertOffset(_ raw: String) -> String? {
        var sign: Character?, out = ""
        for part in raw.split(whereSeparator: \.isWhitespace) {
            var body = String(part), ps: Character = "+"
            if body.first == "+" || body.first == "-" { ps = body.removeFirst() }
            if let sign, sign != ps { return nil }; sign = ps
            let c = body.unicodeScalars.map { Character(String($0)) }; var i = 0
            while i < c.count {
                let start = i; while i < c.count && c[i].isASCII && c[i].isNumber { i += 1 }
                guard i > start && i < c.count, let unit = ["m":"min","M":"m","h":"h","H":"h","d":"d","D":"d","w":"w","W":"w","y":"y","Y":"y"][String(c[i])] else { return nil }
                out += String(c[start..<i])+unit; i += 1
            }
        }; return out.isEmpty ? nil : String(sign ?? "+")+out
    }
}
extension String { var nilIfEmpty: String? { isEmpty ? nil : self } }
