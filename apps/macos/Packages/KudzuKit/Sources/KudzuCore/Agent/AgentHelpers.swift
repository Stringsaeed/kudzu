// Ported from kudzu-core/src/agent/tools.rs
// The tools an agent can call, their JSON schemas, and what they do.

import Foundation

extension Agent {
    static func string(_ args: [String:Any], _ key: String) throws -> String? {
        guard let value = args[key], !(value is NSNull) else { return nil }
        guard let value = value as? String else { throw Failure("`\(key)` must be a string.") }; return value
    }
    static func boolean(_ args: [String:Any], _ key: String) throws -> Bool? {
        guard let value = args[key], !(value is NSNull) else { return nil }
        guard let n = value as? NSNumber, String(cString:n.objCType) == "c" else { throw Failure("`\(key)` must be true or false.") }; return n.boolValue
    }
    static func count(_ args: [String:Any], _ key: String, _ fallback: Int, _ max: Int) throws -> Int {
        guard let value = args[key], !(value is NSNull) else { return fallback }
        guard let n = value as? NSNumber, !["c","d","f"].contains(String(cString:n.objCType)), let integer = UInt64(n.stringValue) else { throw Failure("`\(key)` must be a whole number.") }
        return min(Int(clamping:integer),max)
    }
    static func parseID(_ raw: String, _ what: String) throws -> UUID {
        guard let id = Sync.rustUUID(raw.trimmingCharacters(in:.whitespacesAndNewlines)) else { throw Failure("“\(raw)” isn't a \(what) id. Ids come from the list tools.") }; return id
    }
    static func groupPath(_ groups: [Group], _ id: UUID?) -> String? {
        var names: [String] = [], seen: Set<UUID> = [], cur = id
        while let id = cur, let g = groups.first(where: { $0.id == id }), seen.insert(id).inserted { names.append(g.name); cur = g.parentID }
        return names.isEmpty ? nil : names.reversed().joined(separator:"/")
    }
    static func findGroup(_ groups: [Group], _ query: String) throws -> UUID? {
        let q = query.trimmingCharacters(in:.whitespacesAndNewlines)
        if let id = Sync.rustUUID(q) { guard groups.contains(where: { $0.id == id }) else { throw Failure("No group has the id \(q).") }; return id }
        let wanted = q.trimmingCharacters(in:CharacterSet(charactersIn:"/")).lowercased()
        if let g = groups.first(where: { groupPath(groups,$0.id)?.lowercased() == wanted }) { return g.id }
        let named = groups.filter { $0.name.lowercased() == wanted }
        if named.count > 1 { throw Failure("More than one group is called “\(q)”. Use its path or id from list_groups.") }; return named.first?.id
    }
    static func ensureGroup(_ lib: inout Library, _ query: String) throws -> UUID {
        if let id = try findGroup(lib.groups,query) { return id }
        var parent: UUID?
        for name in query.components(separatedBy:"/").map({ $0.trimmingCharacters(in:.whitespacesAndNewlines) }).filter({ !$0.isEmpty }) {
            if let g = lib.groups.first(where: { $0.parentID == parent && $0.name.lowercased() == name.lowercased() }) { parent = g.id }
            else {
                let next = lib.groups.filter { $0.parentID == parent }.map { $0.sortIndex+1 }.max() ?? 0
                let g = Group(name:name,parentID:parent,sortIndex:next); lib.groups.append(g); parent = g.id
            }
        }
        guard let parent else { throw Failure("The group name is empty.") }; return parent
    }
    static func descendants(_ groups: [Group], _ root: UUID) -> Set<UUID> {
        var set: Set<UUID> = [root]
        while true { let before = set.count; for g in groups where g.parentID.map(set.contains) == true { set.insert(g.id) }; if set.count == before { return set } }
    }
    static func findSnippet(_ lib: Library, _ args: [String:Any]) throws -> Snippet {
        if let raw = try string(args,"id") { let id = try parseID(raw,"snippet"); guard let s = lib.snippets.first(where: { $0.id == id }) else { throw Failure("No snippet has the id \(id.lowercased). It may have been deleted.") }; return s }
        guard let raw = try string(args,"trigger") else { throw Failure("Pass the snippet's `id` or one of its `trigger`s.") }
        let t = raw.trimmingCharacters(in:.whitespacesAndNewlines)
        guard let s = lib.snippets.first(where: { $0.triggers.contains(t) }) ?? lib.snippets.first(where: { $0.triggers.contains { $0.lowercased() == t.lowercased() } }) else { throw Failure("No snippet has the trigger “\(t)”.") }; return s
    }
    static func truncate(_ text: String, _ max: Int) -> (String,Bool) {
        let cut = text.unicodeScalars.count > max; return (Clipboard.excerpt(text,limit:max) + (cut ? "…" : ""),cut)
    }
    static func summary(_ lib: Library, _ s: Snippet) -> [String:Any] {
        let r = Secrets.redact(s.content)
        var v: [String:Any] = ["id":s.id.lowercased,"name":s.name,"triggers":s.triggers,"group":groupPath(lib.groups,s.groupID) as Any? ?? NSNull(),"enabled":s.isEnabled,"favorite":s.isFavorite,"form":Template.analyze(Template.parse(s.content)).isForm,"preview":truncate(r.text.split(whereSeparator: \.isWhitespace).joined(separator:" "),160).0,"uses":lib.usage.filter { $0.snippetID == s.id }.reduce(0) { $0+$1.count }]
        if !r.isClean { v["redacted"] = r.kinds.map(\.label) }; return v
    }
    static let withheldNote = "Parts that look like a password or key were withheld."
}
