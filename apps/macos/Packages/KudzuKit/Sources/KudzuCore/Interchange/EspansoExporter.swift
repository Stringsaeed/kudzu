import Foundation

extension Espanso {
    /// One auto-loaded match file per group path. Paths are keyed by UTF-8 bytes, so two names
    /// that differ only by normalization stay two files (Rust `BTreeMap<String, _>`).
    public static func export(_ library: Library) -> ExactMap<String> {
        var groups: [UUID:Group] = [:], snippets: [UUID:Snippet] = [:]
        for g in library.groups { groups[g.id] = g }; for s in library.snippets { snippets[s.id] = s }
        func sanitize(_ name: String) -> String {
            // `trim`, replace each `/` scalar, then `trim_start_matches('_')`.
            var cleaned = ""
            for scalar in RustText.trim(name).unicodeScalars { cleaned.unicodeScalars.append(scalar == "/" ? "_" : scalar) }
            let stripped = cleaned.unicodeScalars.drop { $0 == "_" }
            let result = String(String.UnicodeScalarView(stripped))
            return result.isEmpty ? "group" : result
        }
        func path(_ id: UUID?) -> String {
            guard let id, let g = groups[id] else { return "base.yml" }
            var components = [sanitize(g.name)], seen: Set<UUID> = [id], parent = g.parentID
            while let p = parent, let g = groups[p], seen.insert(p).inserted { components.insert(sanitize(g.name),at:0); parent = g.parentID }
            return components.joined(separator:"/")+".yml"
        }
        var files = ExactMap<[YAMLValue]>(), seen: Set<UUID?> = []
        for gid in library.groups.map({ Optional($0.id) }) + [nil] where seen.insert(gid).inserted {
            let members = library.snippets.filter { s in (s.groupID.flatMap { groups[$0]?.id }) == gid }
            for s in members {
                guard let node = exportMatch(s,snippets) else { continue }
                var list = files[path(gid)] ?? []
                list.append(node)
                files[path(gid)] = list
            }
        }
        var out = ExactMap<String>()
        for key in files.keys {
            guard let matches = files[key] else { continue }
            var root = YAMLMap(); root.append("matches",.list(matches))
            out[key] = emit(.map(root),indent:0) + "\n"
        }
        return out
    }
    static func exportMatch(_ s: Snippet, _ snippets: [UUID:Snippet]) -> YAMLValue? {
        guard let first = s.triggers.first else { return nil }
        var out = YAMLMap()
        if s.triggers.count == 1 { out.append("trigger",.string(first)) } else { out.append("triggers",.list(s.triggers.map { .string($0) })) }
        if !s.name.isEmpty && !s.name.isExactly(first) { out.append("label",.string(s.name)) }
        let nodes = Template.parse(s.content).nodes
        let names = nodes.compactMap { n -> String? in if case .tag(let t) = n { t.name } else { nil } }
        let shorthand = names.contains { ["field","choice"].contains($0) } && !names.contains { ["date","time","clipboard","snippet"].contains($0) }
        var text = "", fields = YAMLMap(), vars: [YAMLValue] = [], counter = 0, seen: [String] = [], layout: [String] = []
        func escape(_ text: String) -> String { text.replacingOccurrences(of:"{{",with:"\\{\\{") }
        func firstTime(_ name: String) -> Bool {
            if seen.contains(where: { $0.isExactly(name) }) { return false }
            seen.append(name); return true
        }
        for n in nodes {
            guard case .tag(let t) = n else { if case .text(let literal,_) = n { text += escape(literal) }; continue }
            let typed = t.typed
            switch typed {
            case .cursor: text += "$|$"
            case .field(let name,_,_), .choice(let name,_,_):
                text += shorthand ? "[[\(name)]]" : "{{form1.\(name)}}"
                if firstTime(name) { layout.append("\(name): [[\(name)]]"); if let cfg = fieldConfig(typed) { fields.append(name,cfg) } }
            case .date(let format,_,let locale), .time(let format,_,let locale):
                counter += 1; let name = "date_\(counter)"; var params = YAMLMap(); params.append("format",.string(Strftime.toStrftime(format)))
                if let raw = t.string("offset"), let seconds = Strftime.kudzuOffsetToSeconds(raw), seconds != 0 { params.append("offset",.int(seconds)) }
                if let locale, !locale.isEmpty { params.append("locale",.string(locale)) }
                var variable = YAMLMap(); variable.append("name",.string(name)); variable.append("type",.string("date")); variable.append("params",.map(params))
                vars.append(.map(variable)); text += "{{\(name)}}"
            case .clipboard:
                counter += 1; let name = "clipboard_\(counter)"
                var variable = YAMLMap(); variable.append("name",.string(name)); variable.append("type",.string("clipboard"))
                vars.append(.map(variable)); text += "{{\(name)}}"
            case .snippet(let id):
                if let trigger = snippets[id]?.triggers.first {
                    counter += 1; let name = "match_\(counter)"; var params = YAMLMap(); params.append("trigger",.string(trigger))
                    var variable = YAMLMap(); variable.append("name",.string(name)); variable.append("type",.string("match")); variable.append("params",.map(params))
                    vars.append(.map(variable)); text += "{{\(name)}}"
                } else { text += escape(Template.serialize([n])) }
            case .unknown: text += escape(Template.serialize([n]))
            }
        }
        if shorthand { out.append("form",.string(text)); if !fields.pairs.isEmpty { out.append("form_fields",.map(fields)) } }
        else {
            if !seen.isEmpty {
                var params = YAMLMap(); params.append("layout",.string(layout.joined(separator:"\n"))); if !fields.pairs.isEmpty { params.append("fields",.map(fields)) }
                var form = YAMLMap(); form.append("name",.string("form1")); form.append("type",.string("form")); form.append("params",.map(params))
                vars.insert(.map(form),at:0)
            }
            out.append("replace",.string(text)); if !vars.isEmpty { out.append("vars",.list(vars)) }
        }
        if s.fireMode == .wholeWord { out.append("word",.bool(true)) }; if s.caseMode == .propagate { out.append("propagate_case",.bool(true)) }
        if s.insertion == .paste { out.append("force_mode",.string("clipboard")) }; if s.insertion == .type { out.append("force_mode",.string("keys")) }
        return .map(out)
    }
    static func fieldConfig(_ tag: TemplateTag) -> YAMLValue? {
        switch tag {
        case .field(_,let value,let multi):
            var out = YAMLMap(); if multi { out.append("multiline",.bool(true)) }; if let value, !value.isEmpty { out.append("default",.string(value)) }
            return out.pairs.isEmpty ? nil : .map(out)
        case .choice(_,let options,let value):
            var out = YAMLMap(); out.append("type",.string("choice")); out.append("values",.list(options.map { .string($0) }))
            if let value, !value.isEmpty { out.append("default",.string(value)) }; return .map(out)
        default: return nil
        }
    }
    /// YAML written in insertion order, matching `yaml_rust2`'s linked hash (not sorted keys).
    static func emit(_ value: YAMLValue, indent: Int) -> String {
        let pad = String(repeating:" ",count:indent)
        switch value {
        case .map(let map):
            return map.pairs.map { key, v in
                if v.isCollection { return pad + key + ":\n" + emit(v,indent:indent+2) }
                if case .string(let text) = v, text.contains("\n") { return pad + key + ": " + blockYAML(text,indent:indent+2) }
                return pad + key + ": " + scalarYAML(v)
            }.joined(separator:"\n")
        case .list(let list):
            if list.isEmpty { return pad + "[]" }
            return list.map { v in
                if v.isCollection { return pad + "-\n" + emit(v,indent:indent+2) }
                if case .string(let text) = v, text.contains("\n") { return pad + "- " + blockYAML(text,indent:indent+2) }
                return pad + "- " + scalarYAML(v)
            }.joined(separator:"\n")
        default: return pad + scalarYAML(value)
        }
    }
    static func blockYAML(_ text: String, indent: Int) -> String {
        let trailing = text.reversed().prefix { $0 == "\n" }.count
        let marker = trailing == 0 ? "|-" : trailing == 1 ? "|" : "|+"
        var lines = text.components(separatedBy:"\n")
        if trailing > 0 { lines.removeLast() }
        return marker+"\n"+lines.map { String(repeating:" ",count:indent)+$0 }.joined(separator:"\n")
    }
    static func scalarYAML(_ value: YAMLValue) -> String {
        switch value {
        case .string(let s):
            // Double quotes for actual newlines; single quotes preserve backslashes verbatim.
            if s.unicodeScalars.contains("\n") || s.unicodeScalars.contains("\r") { return Sync.json([s]).map { String($0.unicodeScalars.dropFirst().dropLast()) } ?? "''" }
            var body = ""
            for scalar in s.unicodeScalars {
                if scalar == "'" { body.unicodeScalars.append("'") }
                body.unicodeScalars.append(scalar)
            }
            return "'" + body + "'"
        case .bool(let b): return b ? "true" : "false"
        case .int(let n): return String(n)
        default: return "''"
        }
    }
}

struct YAMLMap: Equatable, Sendable {
    var pairs: [(String,YAMLValue)] = []
    mutating func append(_ key: String, _ value: YAMLValue) { pairs.append((key,value)) }
    static func == (lhs: YAMLMap, rhs: YAMLMap) -> Bool {
        lhs.pairs.count == rhs.pairs.count && zip(lhs.pairs, rhs.pairs).allSatisfy { $0.0.isExactly($1.0) && $0.1 == $1.1 }
    }
}

enum YAMLValue: Equatable, Sendable {
    case string(String), bool(Bool), int(Int64), map(YAMLMap), list([YAMLValue])
    var isCollection: Bool { if case .map = self { true } else if case .list = self { true } else { false } }
    static func == (lhs: YAMLValue, rhs: YAMLValue) -> Bool {
        switch (lhs, rhs) {
        case (.string(let a), .string(let b)): a.isExactly(b)
        case (.bool(let a), .bool(let b)): a == b
        case (.int(let a), .int(let b)): a == b
        case (.map(let a), .map(let b)): a == b
        case (.list(let a), .list(let b)): a.count == b.count && zip(a, b).allSatisfy { $0 == $1 }
        default: false
        }
    }
}
