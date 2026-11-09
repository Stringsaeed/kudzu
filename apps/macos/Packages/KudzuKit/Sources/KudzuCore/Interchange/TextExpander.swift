// Ported from kudzu-core/src/interchange/textexpander/mod.rs
// TextExpander export → Kudzu library.
//
// Reads the two formats TextExpander writes: the XML property list behind `.textexpander`
// group files (`snippetsTE2`), and CSV. Macros (`%clipboard`, `%|`, `%Y-%m-%d`,
// `%filltext:…%`, `%snippet:…%`) become Kudzu tags. See the TextExpander mapping table in
// docs/TEMPLATE_SYNTAX.md.

import Foundation

/// TextExpander XML property lists and CSV exports, with macro conversion.
public enum TextExpander {
    struct Raw { var id = UUID(); var index: Int; var abbreviation: String; var content: String; var label: String?; var kind = 0; var exact = false; var created: Date?; var modified: Date? }
    struct RawGroup { var name: String; var snippets: [Raw] }
    static let notTextExpander = "No TextExpander snippets were found in this file. Export a group from TextExpander as .textexpander or CSV."
    /// Reads a TextExpander export (`.textexpander` property list or CSV) from `path`.
    public static func importFile(_ path: URL) throws -> ImportResult {
        if Espanso.isDirectory(path) { throw InterchangeError.invalidSource("Choose the exported .textexpander or .csv file, not a folder.") }
        return try importData(Data(contentsOf:path),fileName:path.lastPathComponent)
    }
    /// Converts the bytes of a TextExpander export. `file_name` names the group and labels warnings.
    public static func importData(_ input: Data, fileName: String) throws -> ImportResult {
        let data = input.starts(with:[0xef,0xbb,0xbf]) ? Data(input.dropFirst(3)) : input
        if data.starts(with:Data("bplist".utf8)) { throw InterchangeError.invalidSource("This is a binary TextExpander file Kudzu can't read. In TextExpander, choose File ▸ Export and pick CSV instead.") }
        if data.starts(with:[80,75,3,4]) { throw InterchangeError.invalidSource("This is a compressed TextExpander file Kudzu can't read. In TextExpander, choose File ▸ Export and pick CSV instead.") }
        let text = String(decoding:data,as:UTF8.self), stem = URL(fileURLWithPath:fileName).deletingPathExtension().lastPathComponent.nilIfEmpty ?? "TextExpander", head = RustText.trim(text)
        var warnings: [ImportWarning] = []; let groups: [RawGroup]
        if ["<?xml","<!DOCTYPE","<plist"].contains(where: { RustText.hasPrefix(head, $0) }) {
            let root: XMLPlist
            do { root = try XMLPlist.parse(text) } catch { throw InterchangeError.invalidSource("Couldn't read this TextExpander file: \(error).") }
            groups = try readPlist(root,stem,fileName,&warnings)
        } else { groups = try readCSV(CSV.parse(text),stem,fileName,&warnings) }
        return build(groups,fileName,warnings)
    }
    static func warning(_ file: String, _ index: Int, _ trigger: String?, _ message: String) -> ImportWarning { ImportWarning(file:file,matchIndex:index,trigger:trigger?.nilIfEmpty,message:message) }
    static func readPlist(_ root: XMLPlist, _ stem: String, _ file: String, _ warnings: inout [ImportWarning]) throws -> [RawGroup] {
        var groups: [RawGroup] = [], next = 0
        func add(_ value: XMLPlist, _ fallback: String) {
            guard let list = value["snippetsTE2"]?.items else { return }
            let name = (value["groupInfo"]?["groupName"] ?? value["groupName"])?.text.map(RustText.trim)?.nilIfEmpty ?? fallback
            var snippets: [Raw] = []
            for item in list {
                let index = next; next += 1
                let abbreviation = item["abbreviation"]?.text
                guard let abbr = abbreviation, let content = item["plainText"]?.text else { warnings.append(warning(file,index,abbreviation,"Snippet is missing its abbreviation or text and was skipped.")); continue }
                let type = item["snippetType"]?.integer ?? 0
                snippets.append(Raw(index:index,abbreviation:abbr,content:XMLPlist.normalize(content),label:item["label"]?.text,kind:type >= 2 ? 2 : type == 1 ? 1 : 0,exact:item["abbreviationMode"]?.integer == 0,created:item["creationDate"]?.dateText.flatMap(KudzuDate.parse),modified:item["modificationDate"]?.dateText.flatMap(KudzuDate.parse)))
            }; groups.append(RawGroup(name:name,snippets:snippets))
        }
        switch root {
        case .dict:
            if root["snippetsTE2"] != nil { add(root,stem) }
            else if let list = root["groups"]?.items { for (i,g) in list.enumerated() { add(g,"\(stem) \(i+1)") } }
        case .array: add(.dict([("snippetsTE2",root)]),stem)
        default: break
        }
        if groups.isEmpty { throw InterchangeError.invalidSource(notTextExpander) }; return groups
    }
    static func readCSV(_ rows: [[String]], _ stem: String, _ file: String, _ warnings: inout [ImportWarning]) throws -> [RawGroup] {
        guard let first = rows.first else { throw InterchangeError.invalidSource(notTextExpander) }
        func find(_ names: [String]) -> Int? { first.firstIndex { names.contains(RustText.trim($0).lowercased()) } }
        let ac = find(["abbreviation","abbrev","trigger","shortcut"]), cc = find(["snippet","content","expansion","text","plain text","plaintext","replacement"]), header = ac != nil && cc != nil
        let a = header ? ac ?? 0 : 0, c = header ? cc ?? 1 : 1, label = header ? find(["label","name","title","description"]) : rows.contains { $0.count > 2 } ? 2 : nil, group = header ? find(["group","group name","folder"]) : nil
        var groups: [RawGroup] = []
        for (index,row) in (header ? Array(rows.dropFirst()) : rows).enumerated() {
            func cell(_ col: Int) -> String { col < row.count ? row[col] : "" }
            let abbr = cell(a), content = cell(c)
            if content.isEmpty { warnings.append(warning(file,index,abbr,"Row has no content and was skipped.")); continue }
            let trimmedGroup = group.map { RustText.trim(cell($0)) }
            let name = (trimmedGroup?.isEmpty == false ? trimmedGroup : nil) ?? stem
            let pos = groups.firstIndex { $0.name.isExactly(name) } ?? groups.count
            if pos == groups.count { groups.append(RawGroup(name:name,snippets:[])) }
            groups[pos].snippets.append(Raw(index:index,abbreviation:RustText.trim(abbr),content:XMLPlist.normalize(content),label:label.map(cell)))
        }
        if groups.isEmpty && warnings.isEmpty { throw InterchangeError.invalidSource(notTextExpander) }; return groups
    }
    static func build(_ groups: [RawGroup], _ file: String, _ initial: [ImportWarning]) -> ImportResult {
        var byAbbr = ExactMap<UUID>(), warnings = initial
        for g in groups { for s in g.snippets where s.kind != 2 && !s.content.isEmpty && !s.abbreviation.isEmpty && byAbbr[s.abbreviation] == nil { byAbbr[s.abbreviation] = s.id } }
        let now = Date(timeIntervalSince1970:floor(Date().timeIntervalSince1970)); var library = Library()
        for g in groups {
            let group = Group(name:g.name,sortIndex:library.groups.count); var any = false
            for s in g.snippets {
                if s.kind == 2 { warnings.append(warning(file,s.index,s.abbreviation,"Script snippet (AppleScript, JavaScript or shell) isn't supported and was skipped.")); continue }
                if s.content.isEmpty { warnings.append(warning(file,s.index,s.abbreviation,"Snippet has no text and was skipped.")); continue }
                var problems: [String] = []; let content = convert(s.content,abbreviations:byAbbr,problems:&problems)
                if s.kind == 1 { problems.append("Rich text formatting was dropped; the snippet was imported as plain text.") }
                for message in problems { warnings.append(warning(file,s.index,s.abbreviation,message)) }
                let name = s.label.map(RustText.trim)?.nilIfEmpty ?? s.abbreviation.nilIfEmpty ?? Raycast.firstLine(s.content), enabled = !s.abbreviation.isEmpty
                if !enabled { warnings.append(warning(file,s.index,nil,"'\(name)' has no abbreviation, so it was imported turned off. Add a trigger to use it.")) }
                let created = s.created.map { Date(timeIntervalSince1970:floor($0.timeIntervalSince1970)) } ?? now
                let updated = (s.modified ?? s.created).map { Date(timeIntervalSince1970:floor($0.timeIntervalSince1970)) } ?? now
                library.snippets.append(Snippet(id:s.id,name:name,triggers:enabled ? [s.abbreviation] : [],content:content,groupID:group.id,caseMode:s.exact ? .exact : .propagate,isEnabled:enabled,createdAt:created,updatedAt:updated)); any = true
            }; if any { library.groups.append(group) }
        }; return ImportResult(library:library,warnings:warnings)
    }
}
