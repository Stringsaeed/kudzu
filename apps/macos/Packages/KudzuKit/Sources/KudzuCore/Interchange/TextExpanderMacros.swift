// Ported from kudzu-core/src/interchange/textexpander/mod.rs
// TextExpander export → Kudzu library.
//
// Reads the two formats TextExpander writes: the XML property list behind `.textexpander`
// group files (`snippetsTE2`), and CSV. Macros (`%clipboard`, `%|`, `%Y-%m-%d`,
// `%filltext:…%`, `%snippet:…%`) become Kudzu tags. See the TextExpander mapping table in
// docs/TEMPLATE_SYNTAX.md.

import Foundation

extension TextExpander {
    static func dateSpec(_ c: [Character], _ i: Int) -> (String,Int)? {
        guard i+1 < c.count && c[i] == "%" else { return nil }
        if c[i+1] == "1" { guard i+2 < c.count, let t = ["m":"M","d":"d","H":"H","I":"h"][String(c[i+2])] else { return nil }; return (t,3) }
        return Strftime.specifiers[c[i+1]].map { ($0,2) }
    }
    static func dateRun(_ c: [Character], _ from: Int) -> (String,Int)? {
        guard let (first,len) = dateSpec(c,from) else { return nil }; var icu = first, end = from+len
        while true {
            var k = end, sep = ""
            while sep.count < 2 && k < c.count && " -/:,.".contains(c[k]) { sep.append(c[k]); k += 1 }
            guard let (token,len) = dateSpec(c,k) else { break }; icu += Strftime.escapeICULiteral(sep)+token; end = k+len
        }; return (icu,end)
    }
    static func dateMath(_ c: [Character], _ from: Int) -> (String,Int)? {
        guard from < c.count && (c[from] == "+" || c[from] == "-") else { return nil }
        var i = from+1; let start = i
        while i < c.count && c[i].isASCII && c[i].isNumber { i += 1 }
        guard i > start && i < c.count, let unit = ["Y":"y","M":"m","W":"w","D":"d","h":"h","m":"min"][String(c[i])] else { return nil }
        return (String(c[from])+String(c[start..<i])+unit,i+1)
    }
    static func fillTag(_ kind: String, _ body: String) -> String? {
        var name: String?, value: String?, options: [String] = []
        for part in RustText.split(body, separator: ":", omittingEmptySubsequences: false) {
            if RustText.hasPrefix(part, "name=") { name = RustText.trim(RustText.dropFirst(part, 5)) }
            else if RustText.hasPrefix(part, "default=") { value = RustText.dropFirst(part, 8) }
            else if ["width=","height=","scrollbar=","hscroll="].contains(where: { RustText.hasPrefix(part, $0) }) { continue }
            else if kind == "fillpopup" && !part.isEmpty { options.append(part) }
        }
        guard let name, !name.isEmpty else { return nil }
        if kind == "fillpopup" && !options.isEmpty {
            if let value, !options.contains(where: { $0.isExactly(value) }) { options.insert(value, at: 0) }
            return ImportTags.choice(name, options, value)
        }
        return ImportTags.field(name, value, kind == "fillarea")
    }
    static func convert(_ raw: String, abbreviations: ExactMap<UUID> = .init(), problems: inout [String]) -> String {
        let c = raw.unicodeScalars.map { Character(String($0)) }; var out = TemplateOut(), i = 0
        func at(_ p: String, _ index: Int) -> Bool { let a = Array(p); return index+a.count <= c.count && Array(c[index..<(index+a.count)]) == a }
        func close(_ from: Int) -> Int? { from < c.count ? (from..<c.count).first { c[$0] == "%" } : nil }
        while i < c.count {
            if c[i] != "%" { out.pushChar(c[i]); i += 1; continue }
            if at("%|",i) { out.tag(ImportTags.cursor()); i += 2; continue }
            if at("%clipboard",i) && !(i+10 < c.count && (c[i+10].isLetter || c[i+10].isNumber || c[i+10] == "_")) { out.tag(ImportTags.clipboard()); i += 10; continue }
            if at("%snippet:",i), let end = close(i+9) {
                let name = RustText.trim(String(c[(i+9)..<end])), source = String(c[i...end])
                if let id = abbreviations[name] { out.tag(ImportTags.snippet(id)) } else { problems.append("\(source) refers to a snippet that isn't in this export; kept as text."); out.text(source) }; i = end+1; continue
            }
            if at("%fillpart:",i), let end = close(i+10) {
                let start = end+1, marker = "%fillpartend%", sectionEnd = start < c.count ? (start..<c.count).first { at(marker,$0) } : nil
                problems.append("Optional sections (%fillpart…%) aren't supported; the section is always included.")
                if let sectionEnd { out.tag(convert(String(c[start..<sectionEnd]),abbreviations:abbreviations,problems:&problems)); i = sectionEnd+marker.count } else { i = start }; continue
            }
            if at("%fillpartend%",i) { i += 13; continue }
            if let kind = ["filltext","fillarea","fillpopup"].first(where: { at("%\($0):",i) }), let end = close(i+2+kind.count) {
                let source = String(c[i...end]), body = String(c[(i+2+kind.count)..<end])
                if let tag = fillTag(kind,body) { out.tag(tag) } else { problems.append("\(source) has no name; kept as text."); out.text(source) }; i = end+1; continue
            }
            if let name = ["key","calc","mouse"].first(where: { at("%\($0):",i) }), let end = close(i+2+name.count) {
                let source = String(c[i...end]); problems.append("%\(name):…% macros aren't supported; \(source) kept as text."); out.text(source); i = end+1; continue
            }
            if at("%@",i) {
                if let (offset,after) = dateMath(c,i+2), let (icu,end) = dateRun(c,after) { out.tag(ImportTags.date(icu,offset)); i = end; continue }
                problems.append("Date math (%@…) isn't supported here; the macro was kept as text.")
            }
            if let (icu,end) = dateRun(c,i) { out.tag(ImportTags.date(icu)); i = end; continue }
            out.pushChar("%"); i += 1
        }; return out.finish()
    }
}
