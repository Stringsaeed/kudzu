import Foundation
import Testing
@testable import KudzuCore

@Suite struct EspansoTests {
    func fixture() throws -> URL {
        let dir = try t2Temp(), c = try t2ImportCase("espanso")
        for (path,text) in c.files ?? [:] { let url = dir.appendingPathComponent(path); try FileManager.default.createDirectory(at:url.deletingLastPathComponent(),withIntermediateDirectories:true); try Data(text.utf8).write(to:url) }; return dir
    }
    func byTrigger(_ r: ImportResult) -> [String:Snippet] { Dictionary(r.library.snippets.compactMap { s in s.triggers.first.map { ($0,s) } },uniquingKeysWith: { a,_ in a }) }
    func normalized(_ source: String, _ library: Library) -> String {
        var s = source; for snippet in library.snippets { if let trigger = snippet.triggers.first { s = s.replacingOccurrences(of:snippet.id.uuidString,with:trigger).replacingOccurrences(of:snippet.id.lowercased,with:trigger) } }; return s
    }
    @Test func goldenDirectoryImport() throws {
        let dir = try fixture(); defer { try? FileManager.default.removeItem(at:dir) }; let r = try Espanso.importDir(dir), lib = r.library, t = byTrigger(r)
        #expect(lib.groups.map(\.name) == ["base","_private","dynamic","forms","unsupported","work","meetings"])
        let work = try #require(lib.groups.first { $0.name == "work" }), meetings = try #require(lib.groups.first { $0.name == "meetings" }); #expect(work.parentID == nil); #expect(meetings.parentID == work.id)
        #expect(t[":sig"]?.name == "Email Signature"); #expect(t[":sig"]?.content == "Best regards,\nJane Doe\n"); #expect(t[":sig"]?.caseMode == .exact); #expect(t[":hello"]?.triggers == [":hello",":hi"]); #expect(t[":hello"]?.content == "Hello there!"); #expect(t[":hello"]?.fireMode == .wholeWord && t[":hello"]?.caseMode == .propagate)
        #expect(t["ther"]?.fireMode == .wholeWord && t["ther"]?.caseMode == .propagate); #expect(t[":div"]?.name == "HTML Div"); #expect(t[":div"]?.content == "<div>{{cursor}}</div>"); #expect(t[":div"]?.insertion == .type); #expect(t[":escaped"]?.content == "Literal braces \\{{not_a_var}} here")
        #expect(t[":secret"]?.name == "Imported Secret"); #expect(t[":secret"]?.content == "Loaded from _private.yml on {{date format=\"yyyy-MM-dd\"}}"); #expect(t[":unimported"] == nil)
        #expect(t[":greet"]?.content == #"Hey {{field name="name" default="Friend"}}, {{field name="note" default="Happy Birthday" multiline=true}}!"#)
        #expect(t[":order"]?.content == #"Order: {{choice name="drink" options="Coffee|Tea\\|Green|Juice" default="Coffee"}} ({{choice name="size" options="Small|Medium|Large"}})"#)
        #expect(t[":verbose_form"]?.content == #"Hi {{field name="user" default="Admin"}}, role: {{choice name="role" options="Developer|Designer" default="Developer"}}"#)
        #expect(t[":now"]?.content == #"It's {{date format="HH:mm"}}"#); #expect(t[":tomorrow"]?.content == #"Tomorrow is {{date format="yyyy-MM-dd" offset="+1d" locale="en_US"}}"#)
        #expect(t[":full_date"]?.content == #"{{date format="EEEE, MMMM d, yyyy hh:mm:ss a (EEE MMM dd ''yy 'day' DDD zzz xx) 100%"}}"#)
        #expect(t[":paste_link"]?.content == #"<a href="{{clipboard}}">{{cursor}}</a>"#)
        let sig = try #require(t[":sig"]); #expect(t[":nested_sig"]?.content == "Thanks!\n{{snippet id=\"\(sig.id.uuidString)\"}}")
        #expect(t[":meeting"]?.groupID == meetings.id); #expect(t[":meeting"]?.content == "Let's meet at {{cursor}} tomorrow."); #expect(t[":standup"]?.content == "Yesterday: {{field name=\"yesterday\"}}\nToday: {{field name=\"today\"}}")
        #expect(t[":cat"] == nil); #expect(t[":rich_md"]?.content == "This *text* is **bold**"); #expect(t[":rich_html"]?.content == "<p>Hello <b>world</b></p>")
        for (trigger,content) in [(":ip","IP: \\{{output}}"),(":py","Out: \\{{script_out}}"),(":rand","Quote: \\{{r}}"),(":echo_test","Echo: \\{{e}}"),(":broken_nest","Missing: \\{{missing_match}}" )] { #expect(t[trigger]?.content == content); #expect(r.warnings.contains { $0.trigger == trigger }) }
        #expect(t[":weird_date"]?.content == #"{{date format="yyyy-%'V'-%'u'"}}"#); #expect(r.warnings.contains { $0.trigger == ":weird_date" && $0.message.contains("%V") })
        let cat = try #require(r.warnings.first { $0.trigger == ":cat" }); #expect(cat.file == "unsupported.yml"); #expect(cat.matchIndex == 1); #expect(cat.line == 6)
    }
    @Test func roundTripImportExportImport() throws {
        let dir = try fixture(); defer { try? FileManager.default.removeItem(at:dir) }
        let first = Espanso.importFiles(["base.yml","forms.yml","dynamic.yml","work/meetings.yml"].map { dir.appendingPathComponent("match/"+$0) }); #expect(first.warnings.isEmpty)
        let exported = Espanso.export(first.library); for key in ["base.yml","forms.yml","dynamic.yml","work/meetings.yml"] { #expect(exported[key] != nil) }
        let temp = try t2Temp(); defer { try? FileManager.default.removeItem(at:temp) }; _ = try Coordinator.exportEspansoToDir(first.library,dir:temp); let second = try Espanso.importDir(temp)
        #expect(second.warnings.isEmpty); #expect(second.library.snippets.count == first.library.snippets.count)
        let b = byTrigger(second)
        for (trigger,a) in byTrigger(first) { let s = try #require(b[trigger]); #expect(s.name == a.name); #expect(s.triggers == a.triggers); #expect(s.fireMode == a.fireMode); #expect(s.caseMode == a.caseMode); #expect(s.insertion == a.insertion); #expect(normalized(s.content,second.library) == normalized(a.content,first.library)) }
    }
    @Test func deterministicOrderingAndDefaultPaths() throws {
        let dir = try fixture(); defer { try? FileManager.default.removeItem(at:dir) }; let a = try Espanso.importDir(dir), b = try Espanso.importDir(dir)
        #expect(a.library.groups.map(\.name) == b.library.groups.map(\.name)); #expect(a.library.snippets.map(\.triggers) == b.library.snippets.map(\.triggers)); #expect(a.library.snippets.map { normalized($0.content,a.library) } == b.library.snippets.map { normalized($0.content,b.library) })
        #expect(Espanso.defaultConfigDirs(home:URL(fileURLWithPath:"/Users/tester")).map(\.path) == ["/Users/tester/Library/Application Support/espanso","/Users/tester/.config/espanso"])
    }
    @Test func fixtureImportsParseWithoutErrors() throws { let dir = try fixture(); defer { try? FileManager.default.removeItem(at:dir) }; let r = try Espanso.importDir(dir); #expect(!r.library.snippets.isEmpty); for s in r.library.snippets { #expect(!Template.parse(s.content).diagnostics.contains { $0.severity == .error }) } }
    @Test func literalBackslashesSurviveImportRenderAndExport() throws {
        let dir = try t2Temp(); defer { try? FileManager.default.removeItem(at:dir) }; let yaml = "matches:\n  - trigger: \";path\"\n    replace: 'C:\\Users\\me \\\\server'\n  - trigger: \";bs\"\n    replace: 'ends with backslash \\'\n"; try Data(yaml.utf8).write(to:dir.appendingPathComponent("base.yml"))
        let library = try Espanso.importDir(dir).library; #expect(t2Render(try #require(library.snippets.first { $0.triggers == [";path"] }).content) == #"C:\Users\me \\server"#); #expect(t2Render(try #require(library.snippets.first { $0.triggers == [";bs"] }).content) == #"ends with backslash \"#)
        let again = try t2Temp(); defer { try? FileManager.default.removeItem(at:again) }; _ = try Coordinator.exportEspansoToDir(library,dir:again); #expect(Set(try Espanso.importDir(again).library.snippets.map(\.content)) == Set(library.snippets.map(\.content)))
    }
    @Test func exporterReadsBareNumbersAndKeepsUnrecognizedTags() {
        let exported = Espanso.export(Library(snippets:[Snippet(name:"n",triggers:[";n"],content:"{{field name=qty default=5}}\n{{future size=3}} \\\\server")])), yaml = exported["base.yml"] ?? ""
        #expect(yaml.contains("5")); #expect(yaml.contains(#"\{\{future size=3}}"#)); #expect(yaml.contains(#"\server"#))
    }
    @Test func exporterWritesVarsForDatesClipboardNestedSnippetsAndMixedForms() throws {
        let target = Snippet(name:"Sig",triggers:[";sig"],content:"Regards",caseMode:.exact), mixed = Snippet(name:"Mixed",triggers:[";mix"],content:"On {{date format=\"yyyy-MM-dd\" offset=\"+1d\"}} {{clipboard}} {{snippet id=\"\(target.id)\"}} {{field name=\"who\"}}",fireMode:.wholeWord,insertion:.paste), lib = Library(snippets:[target,mixed]), dir = try t2Temp(); defer { try? FileManager.default.removeItem(at:dir) }
        _ = try Coordinator.exportEspansoToDir(lib,dir:dir); let r = try Espanso.importDir(dir), t = byTrigger(r); #expect(r.warnings.isEmpty)
        #expect(normalized(try #require(t[";mix"]).content,r.library) == normalized(mixed.content,lib)); #expect(t[";mix"]?.fireMode == .wholeWord); #expect(t[";mix"]?.caseMode == .propagate); #expect(t[";mix"]?.insertion == .paste); #expect(t[";sig"]?.caseMode == .exact)
    }
    @Test func groupsBecomeNestedFilesAndSameNamedGroupsShareOne() {
        let parent = Group(name:"Work/Stuff"), child = Group(name:"_Emails",parentID:parent.id), twin = Group(name:"Work/Stuff")
        let snippets = [(child,";a"),(parent,";b"),(twin,";c")].map { Snippet(name:$0.1,triggers:[$0.1],content:"x",groupID:$0.0.id) }, out = Espanso.export(Library(groups:[parent,child,twin],snippets:snippets))
        #expect(out.keys.sorted() == ["Work_Stuff.yml","Work_Stuff/Emails.yml"]); #expect(out["Work_Stuff.yml"]?.contains(";b") == true && out["Work_Stuff.yml"]?.contains(";c") == true)
    }
    @Test func malformedAndMissingInputsWarnInsteadOfFailing() throws {
        let dir = try t2Temp(); defer { try? FileManager.default.removeItem(at:dir) }
        for (name,text) in ["bad.yml":"matches: [unclosed\n","list.yml":"- just\n- a list\n","empty.yml":"# only a comment\n","ok.yml":"imports:\n  - missing.yml\nmatches:\n  - just a string\n  - trigger: ':x'\n    left_word: true\n    replace: 'x'\n  - trigger: ':none'\n"] { try Data(text.utf8).write(to:dir.appendingPathComponent(name)) }
        let r = try Espanso.importDir(dir), messages = r.warnings.map(\.message)
        #expect(messages.contains { $0.hasPrefix("YAML syntax error") }); for m in ["Expected top-level YAML mapping in match file.","Match file not found at 'missing.yml'.","Malformed match entry at index 0 was skipped.","Partial word boundary ('left_word'/'right_word') mapped to whole-word mode.","Match has no replacement content ('replace', 'form', 'markdown', or 'html') and was skipped."] { #expect(messages.contains(m)) }
        #expect(r.library.snippets.count == 1); #expect(r.library.snippets[0].fireMode == .wholeWord); #expect(r.library.groups.contains { $0.name == "empty" })
    }
    @Test func naturalSortIsCaseInsensitiveAndNumeric() { #expect(["b.yml","A10.yml","a2.yml","Work","base.yml"].sorted(by:Espanso.naturalLess) == ["a2.yml","A10.yml","b.yml","base.yml","Work"]) }
    @Test func normalizeAndCommonParent() { #expect(Espanso.normalize(URL(fileURLWithPath:"/a/b/./../c/d.yml")).path == "/a/c/d.yml"); #expect(Espanso.commonParent([URL(fileURLWithPath:"/x/m/base.yml"),URL(fileURLWithPath:"/x/m/work/a.yml")])?.path == "/x/m") }
}
