// Ported from kudzu-core/src/agent/tools.rs
// The tools an agent can call, their JSON schemas, and what they do.

import Foundation

extension Agent {
    static func triggers(_ args: [String:Any]) throws -> [String]? {
        guard let v = args["triggers"], !(v is NSNull) else { return nil }
        if let s = v as? String { return [s] }; if let a = v as? [String] { return a }; throw Failure("`triggers` must be a list of strings.")
    }
    static func caseMode(_ args: [String:Any]) throws -> CaseMode? {
        guard let raw = try string(args,"caseMode") else { return nil }
        guard let mode = CaseMode(rawValue:raw) else { throw Failure("Unknown caseMode “\(raw)”: use propagate or exact.") }; return mode
    }
    static func fireMode(_ args: [String:Any]) throws -> FireMode? {
        guard let raw = try string(args,"fireMode") else { return nil }
        guard let mode = FireMode(rawValue:raw) else { throw Failure("Unknown fireMode “\(raw)”: use immediate or wholeWord.") }; return mode
    }
    static func checkedTriggers(_ lib: Library, _ id: UUID?, _ raw: [String], _ mode: CaseMode) throws -> [String] {
        var accepted: [String] = []
        for raw in raw {
            let t: String
            switch TriggerValidator.validate(raw,existing:accepted,caseSensitive:mode == .exact) {
            case .success(let valid): t = valid
            case .failure(let e): throw Failure("Trigger “\(raw)”: \(e.message)")
            }
            if let other = lib.snippets.first(where: { $0.id != id && $0.triggers.contains { $0.lowercased() == t.lowercased() } }) {
                let name = other.name.isEmpty ? "another snippet" : "“\(other.name)”"
                throw Failure("“\(t)” is already the trigger of \(name) (id \(other.id.lowercased)). Choose another trigger, or edit that snippet instead.")
            }; accepted.append(t)
        }
        if accepted.isEmpty { throw Failure("A snippet needs at least one trigger.") }; return accepted
    }
    static func warnings(_ lib: Library, _ s: Snippet) -> [String] {
        var out = Template.parse(s.content).diagnostics.map(\.message)
        for c in TriggerConflicts.find(snippets:lib.snippets,groups:lib.groups) where c.kind == .shadowing && (c.snippetID == s.id || c.otherSnippetID == s.id) {
            out.append("“\(c.trigger)” fires before “\(c.otherTrigger)” can be typed, so the longer trigger never expands. Consider wholeWord or a different trigger.")
        }; return out
    }
    static func checkContent(_ content: String) throws {
        if content.contains("[redacted:") { throw Failure("The content contains a “[redacted: …]” marker. Secrets can't be written back through an agent; ask the user to edit the snippet in Kudzu.") }
    }
    static func createSnippet(_ lib: inout Library, _ args: [String:Any], _ now: Date) throws -> [String:Any] {
        let name = try string(args,"name")?.trimmingCharacters(in:.whitespacesAndNewlines) ?? ""
        if name.isEmpty { throw Failure("Give the snippet a `name`.") }
        let content = try string(args,"content") ?? ""
        if content.trimmingCharacters(in:.whitespacesAndNewlines).isEmpty { throw Failure("The snippet's `content` is empty.") }; try checkContent(content)
        let mode = try caseMode(args) ?? .propagate
        guard let raw = try triggers(args) else { throw Failure("Give the snippet at least one trigger in `triggers`.") }
        let triggers = try checkedTriggers(lib,nil,raw,mode), fire = try fireMode(args) ?? .immediate, favorite = try boolean(args,"favorite") ?? false
        let group = try string(args,"group")?.trimmingCharacters(in:.whitespacesAndNewlines)
        if let group, !group.isEmpty { _ = try findGroup(lib.groups,group) }
        var s = Snippet(name:name,triggers:triggers,content:content,fireMode:fire,caseMode:mode,isFavorite:favorite,createdAt:now,updatedAt:now)
        if let group, !group.isEmpty { s.groupID = try ensureGroup(&lib,group) }
        lib.snippets.insert(s,at:0)
        var v: [String:Any] = ["created":summary(lib,s)]; let w = warnings(lib,s); if !w.isEmpty { v["warnings"] = w }; return v
    }
    static func updateSnippet(_ lib: inout Library, _ args: [String:Any], _ now: Date) throws -> [String:Any] {
        guard let raw = try string(args,"id") else { throw Failure("Pass the `id` of the snippet to edit.") }
        let id = try parseID(raw,"snippet")
        guard var s = lib.snippets.first(where: { $0.id == id }) else { throw Failure("No snippet has the id \(id.lowercased).") }; let before = s
        if let name = try string(args,"name") {
            let n = name.trimmingCharacters(in:.whitespacesAndNewlines); if n.isEmpty { throw Failure("A snippet's name can't be empty.") }; s.name = n
        }
        if let content = try string(args,"content"), !content.utf8.elementsEqual(s.content.utf8) {
            if Secrets.containsSecret(s.content) { throw Failure("This snippet holds what looks like a password or key, so its content can only be changed in Kudzu. Other fields can still be edited here.") }
            if s.formatting != nil { throw Failure("This is a formatted snippet (bold, links or images). Edit its content in Kudzu so the formatting isn't lost.") }
            if content.trimmingCharacters(in:.whitespacesAndNewlines).isEmpty { throw Failure("The new content is empty.") }
            try checkContent(content); s.content = content
        }
        if let mode = try caseMode(args) { s.caseMode = mode }; if let mode = try fireMode(args) { s.fireMode = mode }
        if let raw = try triggers(args) { s.triggers = try checkedTriggers(lib,id,raw,s.caseMode) }
        if let enabled = try boolean(args,"enabled") { s.isEnabled = enabled }; if let favorite = try boolean(args,"favorite") { s.isFavorite = favorite }
        if args["group"] != nil {
            let group = try string(args,"group")?.trimmingCharacters(in:.whitespacesAndNewlines)
            if let group, !group.isEmpty { _ = try findGroup(lib.groups,group); s.groupID = try ensureGroup(&lib,group) } else { s.groupID = nil }
        }
        if Sync.sameModel(s,before) { return ["updated":summary(lib,s),"unchanged":true] }
        s.updatedAt = now; if let i = lib.snippets.firstIndex(where: { $0.id == id }) { lib.snippets[i] = s }
        var v: [String:Any] = ["updated":summary(lib,s)]; let w = warnings(lib,s); if !w.isEmpty { v["warnings"] = w }; return v
    }
}
