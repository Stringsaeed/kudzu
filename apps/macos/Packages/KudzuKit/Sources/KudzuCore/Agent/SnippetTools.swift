// Ported from kudzu-core/src/agent/tools.rs
// The tools an agent can call, their JSON schemas, and what they do.

import Foundation

extension Agent {
    static func listSnippets(_ lib: Library, _ args: [String:Any], _ access: Access) throws -> [String:Any] {
        let limit = max(1,try count(args,"limit",50,200)), offset = try count(args,"offset",0,Int.max), favorites = try boolean(args,"favoritesOnly") ?? false
        var inGroups: Set<UUID>?
        if let group = try string(args,"group"), !group.trimmingCharacters(in:.whitespacesAndNewlines).isEmpty {
            guard let id = try findGroup(lib.groups,group) else { throw Failure("No group is called “\(group)”. list_groups shows them all.") }; inGroups = descendants(lib.groups,id)
        }
        let query = try string(args,"query")?.trimmingCharacters(in:.whitespacesAndNewlines).lowercased() ?? "", terms = query.split(whereSeparator: \.isWhitespace).map(String.init)
        var hits: [(Int,Int,Snippet)] = []
        for (i,s) in lib.snippets.enumerated() {
            if favorites && !s.isFavorite { continue }; if let inGroups, s.groupID.map(inGroups.contains) != true { continue }
            let name = s.name.lowercased(), ts = s.triggers.map { $0.lowercased() }, content = Secrets.redact(s.content).text.lowercased(), all = [name,ts.joined(separator:" "),content].joined(separator:"\n")
            if !terms.allSatisfy(all.contains) { continue }
            let score = terms.isEmpty ? 0 : ts.contains(query) ? 4 : name.contains(query) ? 3 : ts.contains(where: { $0.contains(query) }) ? 2 : 1
            hits.append((score,i,s))
        }
        hits.sort { $0.0 != $1.0 ? $0.0 > $1.0 : $0.1 < $1.1 }
        let page = hits.dropFirst(offset).prefix(limit).map { summary(lib,$0.2) }
        return ["total":hits.count,"returned":page.count,"triggerPrefix":access.triggerPrefix,"snippets":page]
    }
    static func getSnippet(_ lib: Library, _ args: [String:Any]) throws -> [String:Any] {
        let s = try findSnippet(lib,args), r = Secrets.redact(s.content)
        let byID = Dictionary(lib.snippets.map { ($0.id,Secrets.redact($0.content).text) },uniquingKeysWith: { _,new in new })
        let fields = Template.formFields(r.text,resolveSnippet: { byID[$0] })
        var v = summary(lib,s); v.removeValue(forKey:"preview")
        v["content"] = r.text; v["groupId"] = s.groupID?.lowercased as Any? ?? NSNull(); v["fireMode"] = s.fireMode.rawValue; v["caseMode"] = s.caseMode.rawValue
        v["formatted"] = s.formatting != nil; v["fields"] = fields.map(Sync.object)
        v["createdAt"] = KudzuDate.formatWholeSeconds(s.createdAt); v["updatedAt"] = KudzuDate.formatWholeSeconds(s.updatedAt)
        v["lastUsedAt"] = lib.usage.filter { $0.snippetID == s.id }.compactMap(\.lastUsedAt).max().map(KudzuDate.formatWholeSeconds) as Any? ?? NSNull()
        if !r.isClean { v["note"] = withheldNote }; return v
    }
    static func expandSnippet(_ lib: Library, _ args: [String:Any], _ env: RenderEnv) throws -> [String:Any] {
        let s = try findSnippet(lib,args); var given: [String:String] = [:]
        if let value = args["values"], !(value is NSNull) {
            guard let values = value as? [String:Any] else { throw Failure("`values` must be an object of field names to text.") }
            for (key,value) in values {
                if let text = value as? String { given[key] = text }
                else if let n = value as? NSNumber { given[key] = String(cString:n.objCType) == "c" ? (n.boolValue ? "true" : "false") : n.stringValue }
                else { throw Failure("The value for “\(key)” must be text.") }
            }
        }
        let byID = Dictionary(lib.snippets.map { ($0.id,Secrets.redact($0.content).text) },uniquingKeysWith: { _,new in new }), source = byID[s.id] ?? ""
        let fields = Template.formFields(source,resolveSnippet: { byID[$0] }); var values = FormMemory().initialValues(for:s.id,fields:fields), missing: [String] = []
        for f in fields {
            if let value = given.removeValue(forKey:f.name) {
                if f.kind == .choice && !f.options.contains(value) { throw Failure("“\(value)” isn't an option for “\(f.name)”. Choose one of: \(f.options.joined(separator:", ")).") }; values[f.name] = value
            } else if f.kind != .choice && f.defaultValue == nil { missing.append(f.name) }
        }
        let context = RenderContext(now:env.now,timeZone:env.timeZone,locale:Locale(identifier:env.locale),clipboard:env.clipboard.map { Secrets.redact($0).text },formValues:values,resolveSnippet: { byID[$0] },rootSnippetID:s.id)
        let r = Secrets.redact(Template.render(source,context:context).text)
        var v: [String:Any] = ["id":s.id.lowercased,"name":s.name,"text":r.text,"fields":fields.map(Sync.object)]
        if !missing.isEmpty { v["emptyFields"] = missing }; if !given.isEmpty { v["unknownFields"] = given.keys.sorted() }
        if r.text.contains("[redacted:") { v["note"] = withheldNote }; return v
    }
    static func listGroups(_ lib: Library) -> [String:Any] {
        let groups = lib.groups.map { g -> [String:Any] in
            ["id":g.id.lowercased,"name":g.name,"path":groupPath(lib.groups,g.id) as Any? ?? NSNull(),"parentId":g.parentID?.lowercased as Any? ?? NSNull(),"enabled":g.isEnabled,"snippets":lib.snippets.filter { $0.groupID == g.id }.count]
        }.sorted { ($0["path"] as? String ?? "").lowercased() < ($1["path"] as? String ?? "").lowercased() }
        return ["groups":groups,"ungroupedSnippets":lib.snippets.filter { $0.groupID == nil }.count]
    }
}
