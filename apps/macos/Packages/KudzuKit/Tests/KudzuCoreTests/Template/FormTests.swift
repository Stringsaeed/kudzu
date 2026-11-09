import Foundation
import Testing
@testable import KudzuCore

@Test func recursiveFormOrderAndFirstDefinition() {
    let id = UUID()
    let source = "{{field name=a}}{{snippet id=\"\(id)\"}}{{choice name=b options=\"wrong\"}}{{field name=c}}"
    let fields = TemplateAnalysis.formFields(in: source) { nested in
        nested == id ? "{{choice name=b options=\"one|two\"}}{{field name=a multiline=true}}" : nil
    }
    #expect(fields.map(\.name) == ["a", "b", "c"])
    #expect(fields[0].kind == .text)
    #expect(fields[1].options == ["one", "two"])
}

@Test func recursiveFormsStopCyclesAndDepth() {
    let ids = (0..<10).map { _ in UUID() }
    let sources = Dictionary(uniqueKeysWithValues: ids.enumerated().map { index, id in
        (id, "{{field name=f\(index)}}{{snippet id=\"\(ids[(index + 1) % ids.count])\"}}")
    })
    let fields = TemplateAnalysis.formFields(in: sources[ids[0]]!) { sources[$0] }
    #expect(fields.map(\.name) == (0...8).map { "f\($0)" })
    #expect(TemplateAnalysis.formFields(in: "{{snippet id=\"\(ids[0])\"}}") { _ in
        "{{field name=once}}{{snippet id=\"\(ids[0])\"}}"
    }.map(\.name) == ["once"])
}

@Test func formMemoryValidatesAndRoundTrips() throws {
    let id = UUID()
    let fields = TemplateAnalysis.analyze(TemplateParser.parse("{{field name=text default=hello}}{{choice name=pick options=\"a|b\" default=b}}{{choice name=first options=\"x|y\"}}{{field name=empty}}" )).formFields
    var memory = FormMemory()
    #expect(memory.initialValues(for: id, fields: fields) == ["text": "hello", "pick": "b", "first": "x", "empty": ""])
    memory.record(["text": "", "pick": "removed", "first": "y"], for: id)
    let decoded = try JSONDecoder().decode(FormMemory.self, from: JSONEncoder().encode(memory))
    #expect(decoded.initialValues(for: id, fields: fields) == ["text": "", "pick": "b", "first": "y", "empty": ""])
    memory.retainSnippets([UUID()])
    #expect(memory.initialValues(for: id, fields: fields)["text"] == "hello")
}

@Test func formsIgnoreMissingReferencesAndEscapedTags() {
    let source = "\\{{field name=escaped}}{{snippet id=\"\(UUID())\"}}{{field name=visible}}"
    #expect(TemplateAnalysis.formFields(in: source) { _ in nil }.map(\.name) == ["visible"])
}

@Test func formMemoryRejectsObsoleteChoiceDefaults() {
    let id = UUID()
    let fields = TemplateAnalysis.analyze(TemplateParser.parse("{{choice name=pick options=\"a|b\" default=obsolete}}{{choice name=empty options=\"\"}}")).formFields
    var memory = FormMemory()
    memory.record(["pick": "also-obsolete"], for: id)
    #expect(memory.initialValues(for: id, fields: fields)["pick"] == "a")
    #expect(memory.initialValues(for: id, fields: fields)["empty"] == "")
}
