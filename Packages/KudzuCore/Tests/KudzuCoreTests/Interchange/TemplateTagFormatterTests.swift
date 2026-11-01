import Foundation
import Testing
@testable import KudzuCore

/// The importer builds tags with `TemplateTagFormatter`; everything it writes must parse back to the same tag.
@Suite struct TemplateTagFormatterTests {
    private func onlyTag(_ source: String) throws -> KudzuCore.Tag {
        let parsed = TemplateParser.parse(source)
        #expect(parsed.diagnostics.isEmpty, "\(source)")
        #expect(parsed.nodes.count == 1)
        guard case .tag(let tag)? = parsed.nodes.first else { throw TagMissing() }
        return tag
    }
    private struct TagMissing: Error {}

    @Test func buildersMatchTheirHistoricalOutput() {
        #expect(TemplateTagFormatter.cursorTag == "{{cursor}}")
        #expect(TemplateTagFormatter.clipboardTag() == "{{clipboard}}")
        #expect(TemplateTagFormatter.clipboardTag(trim: true) == "{{clipboard trim=true}}")
        #expect(TemplateTagFormatter.dateTag(format: nil) == "{{date}}")
        #expect(TemplateTagFormatter.dateTag(format: "") == "{{date}}")
        #expect(TemplateTagFormatter.dateTag(format: "yyyy", offset: "+1d", locale: "en_US")
            == #"{{date format="yyyy" offset="+1d" locale="en_US"}}"#)
        #expect(TemplateTagFormatter.fieldTag(name: "who") == #"{{field name="who"}}"#)
        #expect(TemplateTagFormatter.fieldTag(name: "who", defaultValue: "x", multiline: true)
            == #"{{field name="who" default="x" multiline=true}}"#)
        let id = UUID()
        #expect(TemplateTagFormatter.snippetTag(id: id) == #"{{snippet id="\#(id.uuidString)"}}"#)
        #expect(TemplateTagFormatter.choiceTag(name: "c", options: ["a", "b"], defaultValue: "b")
            == #"{{choice name="c" options="a|b" default="b"}}"#)
    }

    @Test(arguments: ["plain", "say \"hi\"", #"C:\Users\me"#, #"\\server\share"#, "}} and {{", "line\nbreak", "😀"])
    func attributeTextSurvives(value: String) throws {
        let field = try onlyTag(TemplateTagFormatter.fieldTag(name: value, defaultValue: value))
        #expect(field.typed == .field(name: value, default: value, multiline: false))
        let date = try onlyTag(TemplateTagFormatter.dateTag(format: value, locale: value))
        #expect(date.typed == .date(format: value, offset: nil, locale: value))
    }

    @Test func choiceOptionsKeepBackslashesAndPipes() throws {
        let options = [#"C:\dir"#, "a|b", #"x\y"#, "", "plain"]
        let tag = try onlyTag(TemplateTagFormatter.choiceTag(name: "c", options: options, defaultValue: #"C:\dir"#))
        #expect(tag.typed == .choice(name: "c", options: options, default: #"C:\dir"#))
    }

    @Test func exporterReadsBareNumbersAndKeepsUnrecognizedTags() throws {
        let content = "{{field name=qty default=5}}\n{{future size=3}} \\\\server"
        let snippet = SnippetRecord(name: "n", triggers: [";n"], content: content)
        let library = KudzuLibrary(formatVersion: KudzuLibrary.currentFormatVersion, groups: [], snippets: [snippet], usage: [])
        let yaml = try #require(EspansoExporter.export(library)["base.yml"])
        #expect(yaml.contains("5"))
        #expect(yaml.contains(#"\{\{future size=3}}"#))
        #expect(yaml.contains(#"\server"#))
    }
}
