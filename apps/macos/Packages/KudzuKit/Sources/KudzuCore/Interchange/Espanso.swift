import Foundation

/// Espanso match-file import/export. Unsupported features remain literal with warnings.
public enum Espanso {
    public static func defaultConfigDirs(home: URL) -> [URL] { [home.appendingPathComponent("Library/Application Support/espanso"),home.appendingPathComponent(".config/espanso")] }
    static func isDirectory(_ url: URL) -> Bool { (try? url.resourceValues(forKeys:[.isDirectoryKey]).isDirectory) == true }
    static func normalize(_ url: URL) -> URL { url.standardizedFileURL }
    static func naturalLess(_ a: String, _ b: String) -> Bool {
        let x = Array(a.unicodeScalars), y = Array(b.unicodeScalars); var i = 0, j = 0
        func digit(_ c: UnicodeScalar) -> Bool { (48...57).contains(c.value) }
        while i < x.count && j < y.count {
            if digit(x[i]) && digit(y[j]) {
                let ia = i, jb = j
                while i < x.count && digit(x[i]) { i += 1 }; while j < y.count && digit(y[j]) { j += 1 }
                let lhs = Array(x[ia..<i].drop(while: { $0 == "0" })), rhs = Array(y[jb..<j].drop(while: { $0 == "0" }))
                if lhs.count != rhs.count { return lhs.count < rhs.count }
                if lhs != rhs { return lhs.lexicographicallyPrecedes(rhs,by: { $0.value < $1.value }) }
            } else {
                let lhs = String(x[i]).lowercased(), rhs = String(y[j]).lowercased()
                if lhs != rhs { return lhs.utf8.lexicographicallyPrecedes(rhs.utf8) }; i += 1; j += 1
            }
        }
        if i != x.count || j != y.count { return i == x.count }
        return a.utf8.lexicographicallyPrecedes(b.utf8)
    }
    static func discover(_ root: URL) throws -> [URL] {
        guard FileManager.default.fileExists(atPath:root.path) else { return [] }
        var out: [URL] = []
        let entries = try FileManager.default.contentsOfDirectory(at:root,includingPropertiesForKeys:[.isDirectoryKey]).sorted { naturalLess($0.lastPathComponent,$1.lastPathComponent) }
        for path in entries where !RustText.hasPrefix(path.lastPathComponent, ".") && !RustText.hasPrefix(path.lastPathComponent, "_") {
            if isDirectory(path) { out += try discover(path) }
            else if ["yml","yaml"].contains(path.pathExtension.lowercased()) { out.append(normalize(path)) }
        }; return out
    }
    public static func importDir(_ dir: URL) throws -> ImportResult {
        let sub = dir.appendingPathComponent("match"), root = normalize(isDirectory(sub) ? sub : dir)
        return try performImport(discover(root),base:root)
    }
    static func commonParent(_ files: [URL]) -> URL? {
        guard let first = files.first else { return nil }
        var parts = normalize(first).deletingLastPathComponent().pathComponents
        for file in files.dropFirst() {
            let other = normalize(file).deletingLastPathComponent().pathComponents
            parts = Array(zip(parts,other).prefix { $0 == $1 }.map { $0.0 })
        }
        guard !parts.isEmpty else { return nil }; return URL(fileURLWithPath:NSString.path(withComponents:parts),isDirectory:true)
    }
    public static func importFiles(_ files: [URL]) -> ImportResult { performImport(files,base:commonParent(files)) }
    struct MatchFile { var relative: String; var dirs: [String]; var name: String; var root: YAMLNode }
    struct Pending { var id: UUID; var group: UUID; var file: MatchFile; var match: YAMLNode; var index: Int; var triggers: [String]; var content: String }
    static func performImport(_ initial: [URL], base: URL?) -> ImportResult {
        var files: [MatchFile] = [], warnings: [ImportWarning] = [], visited: Set<String> = []
        func load(_ raw: URL) {
            let path = normalize(raw), canonical = path.resolvingSymlinksInPath().path
            if !visited.insert(canonical).inserted { return }
            let relative = base.flatMap { root in
                let prefix = root.path + "/"
                return RustText.hasPrefix(path.path, prefix) ? RustText.dropFirst(path.path, prefix.unicodeScalars.count) : nil
            } ?? path.lastPathComponent
            func warn(_ message: String, _ line: Int? = nil) { warnings.append(ImportWarning(file:relative,line:line,message:message)) }
            guard FileManager.default.fileExists(atPath:path.path) else { warn("Match file not found at '\(relative)'."); return }
            let text: String
            do { text = try String(contentsOf:path,encoding:.utf8) } catch { warn("Failed to read file: \(error.localizedDescription)"); return }
            let root: YAMLNode
            do { root = try YAML.parse(text) ?? YAMLNode(kind:.mapping([]),line:1) } catch { warn("YAML syntax error: \(error)"); return }
            if root.mapping == nil && root.string != "" { warn("Expected top-level YAML mapping in match file.",root.line); return }
            let file = MatchFile(relative:relative,dirs:RustText.split(relative, separator: "/", omittingEmptySubsequences: false).dropLast().map { $0 },name:path.deletingPathExtension().lastPathComponent,root:root)
            files.append(file)
            for item in root["imports"]?.sequence ?? [] {
                if let name = item.string { load(RustText.hasPrefix(name, "/") ? URL(fileURLWithPath:name) : path.deletingLastPathComponent().appendingPathComponent(name)) }
            }
        }
        for file in initial { load(file) }
        var globals = ExactMap<YAMLNode>()
        func vars(_ node: YAMLNode?) -> ExactMap<YAMLNode> {
            var out = ExactMap<YAMLNode>()
            for v in node?.sequence ?? [] { if let name = v.str("name"), v.str("type") != nil { out[name] = v } }; return out
        }
        for file in files { globals.merge(vars(file.root["global_vars"])) { _,new in new } }
        var library = Library(), groupIDs = ExactMap<UUID>(), pending: [Pending] = [], triggers = ExactMap<UUID>()
        func group(_ parts: [String]) -> UUID? {
            var parent: UUID?
            for i in parts.indices {
                let key = parts[...i].joined(separator:"/")
                if let id = groupIDs[key] { parent = id; continue }
                let next = library.groups.filter { $0.parentID == parent }.count
                let g = Group(name:parts[i],parentID:parent,sortIndex:next); groupIDs[key] = g.id; library.groups.append(g); parent = g.id
            }; return parent
        }
        for file in files {
            guard let gid = group(file.dirs+[file.name]) else { continue }
            for (index,m) in (file.root["matches"]?.sequence ?? []).enumerated() {
                var ts: [String] = []; if let t = m.str("trigger"), !t.isEmpty { ts.append(t) }
                ts += (m["triggers"]?.sequence ?? []).compactMap(\.string).filter { !$0.isEmpty }
                func warn(_ message: String, trigger: String? = nil) { warnings.append(ImportWarning(file:file.relative,line:m.line,matchIndex:index,trigger:trigger ?? ts.first ?? m.str("regex"),message:message)) }
                guard m.mapping != nil else { warn("Malformed match entry at index \(index) was skipped."); continue }
                if let regex = m.str("regex") { warn("Regex trigger '\(regex)' is not supported in v1 and was skipped.",trigger:regex); continue }
                if ts.isEmpty { warn("Match is missing a valid 'trigger' or 'triggers' list and was skipped."); continue }
                if m.str("image_path") != nil { warn("Image match ('image_path') is not supported in v1 and was skipped."); continue }
                let content: String
                if let r = m.str("replace") { content = r }
                else if let f = m.str("form") { content = f }
                else if let md = m.str("markdown") { warn("Markdown match imported as plain text; importing rich text formatting is not supported."); content = md }
                else if let html = m.str("html") { warn("HTML match imported as plain text; importing rich text formatting is not supported."); content = html }
                else { warn("Match has no replacement content ('replace', 'form', 'markdown', or 'html') and was skipped."); continue }
                if (m.bool("left_word") == true || m.bool("right_word") == true) && m.bool("word") != true { warn("Partial word boundary ('left_word'/'right_word') mapped to whole-word mode.") }
                let id = UUID(); for t in ts where triggers[t] == nil { triggers[t] = id }
                pending.append(Pending(id:id,group:gid,file:file,match:m,index:index,triggers:ts,content:content))
            }
        }
        let timestamp = Date(timeIntervalSince1970:floor(Date().timeIntervalSince1970))
        for p in pending {
            var variables = globals; variables.merge(vars(p.file.root["global_vars"])) { _,new in new }
            for (key,v) in vars(p.match["vars"]).elements where v.str("type") != "global" { variables[key] = v }
            var converter = EspansoConverter(vars:variables,triggers:triggers,file:p.file.relative,index:p.index,trigger:p.triggers.first,line:p.match.line)
            let form = p.match.str("form") != nil && p.match.str("replace") == nil
            let content = converter.convert(p.content,fields:form ? p.match["form_fields"] ?? YAMLNode(kind:.mapping([]),line:p.match.line) : nil,line:p.match.line)
            warnings += converter.warnings
            let label = p.match.str("label").map(RustText.trim)
            let mode: InsertionMethod?
            switch p.match.str("force_mode")?.lowercased() { case "clipboard","paste": mode = .paste; case "keys","type": mode = .type; case "auto": mode = .automatic; default: mode = nil }
            library.snippets.append(Snippet(id:p.id,name:label.flatMap { $0.isEmpty ? nil : $0 } ?? p.triggers[0],triggers:p.triggers,content:content,groupID:p.group,fireMode:["word","left_word","right_word"].contains { p.match.bool($0) == true } ? .wholeWord : .immediate,caseMode:p.match.bool("propagate_case") == true ? .propagate : .exact,insertion:mode,createdAt:timestamp,updatedAt:timestamp))
        }
        return ImportResult(library:library,warnings:warnings)
    }
}
