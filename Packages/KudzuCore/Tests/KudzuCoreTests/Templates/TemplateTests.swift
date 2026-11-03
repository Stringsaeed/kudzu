import Foundation
import Testing
@testable import KudzuCore

private func context(_ snippets: [UUID: String] = [:]) -> RenderContext {
    RenderContext(now: Date(timeIntervalSince1970: 1706659200), calendar: Calendar(identifier: .gregorian),
                  timeZone: TimeZone(secondsFromGMT: 0)!, locale: Locale(identifier: "en_US_POSIX"),
                  clipboard: " \nhello \n", formValues: ["who": "Ada"], resolveSnippet: { snippets[$0] })
}

@Test(arguments: [
    ("hello", "hello"), (#"\{{cursor}}"#, "{{cursor}}"),
    ("{{clipboard trim=true}}", "hello"), ("{{field name=who}}", "Ada"),
    (#"{{choice name=x options="a|b" default="b"}}"#, "b"),
    ("{{date}} {{time}}", "2024-01-31 00:00"),
    ("{{date offset=+1m}}", "2024-02-29"), ("{{date offset=oops}}", "2024-01-31"),
    ("{{unknown}}", "{{unknown}}"), ("{{field}}", "{{field}}"),
    (#"{{field name=x default="a}}b"}}"#, "a}}b")
])
func rendering(source: String, expected: String) {
    #expect(TemplateRenderer.render(source, context: context()).text == expected)
}

@Test func rangesAndDiagnostics() {
    let parsed = TemplateParser.parse("😀{{cursor}}{{cursor}}{{field name=x name=y}}")
    #expect(parsed.nodes[0].range == 0..<2)
    #expect(parsed.nodes[1].range == 2..<12)
    #expect(parsed.diagnostics.count == 2)
    #expect(TemplateRenderer.render("😀{{cursor}}x{{cursor}}", context: context()).cursorOffset == 2)
}

@Test(arguments: ["{{", "{{date format=}}", "{{snippet id=nope}}", "{{field name=\"oops}}", "{{DATE}}"])
func malformed(source: String) {
    let result = TemplateRenderer.render(source, context: context())
    #expect(result.text == source)
    #expect(!result.diagnostics.isEmpty)
}

@Test func analysis() {
    let info = TemplateAnalysis.analyze(TemplateParser.parse(#"{{field name=x multiline=true default="hi"}}{{choice name=y options="a\|b|c"}}{{field name=x}}{{clipboard}}{{cursor}}"#))
    #expect(info.isForm && info.hasCursor && info.usesClipboard)
    #expect(info.formFields.map(\.name) == ["x", "y"])
    #expect(info.formFields[0].kind == .multiline)
    #expect(info.formFields[1].options == ["a|b", "c"])
}

@Test func nesting() {
    let id = UUID()
    let tag = "{{snippet id=\"\(id)\"}}"
    let result = TemplateRenderer.render("😀" + tag + "{{cursor}}", context: context([id: "a{{cursor}}{{field name=who}}"]))
    #expect(result.text == "😀aAda")
    #expect(result.cursorOffset == 6)
    #expect(!TemplateRenderer.render(tag, context: context([id: tag])).diagnostics.isEmpty)
    #expect(!TemplateRenderer.render(tag, context: context()).diagnostics.isEmpty)
}

@Test func generatedRoundTrips() {
    let texts = ["", "abc", "😀é", "{{literal}}", "\\{{", "\\", "a\nb"]
    for i in 0..<300 {
        let source = TemplateSerializer.serialize([
            .text(texts[i % texts.count], range: 0..<0),
            .tag(Tag(name: "field", attributes: [Attribute(name: "name", value: .string("x")), Attribute(name: "default", value: .string(texts[(i / 7) % texts.count]))])),
            .tag(Tag(name: "clipboard", attributes: [Attribute(name: "trim", value: .bool(i % 2 == 0))]))
        ])
        let parsed = TemplateParser.parse(source)
        #expect(parsed.diagnostics.isEmpty)
        #expect(TemplateSerializer.serialize(parsed.nodes) == source)
        #expect(TemplateParser.parse(TemplateSerializer.serialize(parsed.nodes)) == parsed)
    }
}

@Test func performanceSanity() {
    let source = String(repeating: String(repeating: "x", count: 40) + "{{clipboard}}", count: 200)
    _ = TemplateRenderer.render(source, context: context())
    let start = ContinuousClock.now
    _ = TemplateParser.parse(source)
    let result = TemplateRenderer.render(source, context: context())
    #expect(!result.text.isEmpty)
    #expect(start.duration(to: .now) < .milliseconds(50))
}

@Test(arguments: [
    ("+3d", "2024-02-03 00:00"), ("-1w", "2024-01-24 00:00"),
    ("+2m", "2024-03-31 00:00"), ("+1y", "2025-01-31 00:00"),
    ("-4h", "2024-01-30 20:00"), ("+30min", "2024-01-31 00:30"),
    ("+1w2d", "2024-02-09 00:00"), ("-1y2m", "2022-11-30 00:00")
])
func calendarOffsets(offset: String, expected: String) {
    let source = "{{date format=\"yyyy-MM-dd HH:mm\" offset=\"\(offset)\"}}"
    #expect(TemplateRenderer.render(source, context: context()).text == expected)
}

@Test(arguments: ["", "+", "1d", "+1", "+1q", "+1d-2h", "+1.5d", "+999999999999999999999d"])
func invalidOffsets(offset: String) { #expect(DateOffset(offset) == nil) }

@Test func localesAndEmptyValues() {
    var ctx = context()
    ctx.locale = Locale(identifier: "de_DE")
    ctx.clipboard = nil
    #expect(TemplateRenderer.render(#"{{date format="MMMM"}}/{{date format="MMMM" locale=en_US_POSIX}}"#, context: ctx).text == "Januar/January")
    #expect(TemplateRenderer.render("{{clipboard}}{{field name=missing}}{{choice name=missing}}", context: ctx).text == "")
    ctx.formValues["x"] = ""
    #expect(TemplateRenderer.render(#"{{field name=x default="fallback"}}"#, context: ctx).text == "")
}

@Test func depthAndRepeatedReferences() {
    let ids = (0..<9).map { _ in UUID() }
    func reference(_ index: Int) -> String { "{{snippet id=\"\(ids[index])\"}}" }
    var snippets: [UUID: String] = [:]
    for i in 0..<8 { snippets[ids[i]] = "x" + reference(i + 1) }
    snippets[ids[8]] = "too deep"
    let result = TemplateRenderer.render(reference(0), context: context(snippets))
    #expect(result.text == String(repeating: "x", count: 8))
    #expect(result.diagnostics.count == 1)
    snippets[ids[7]] = "last"
    let allowed = TemplateRenderer.render(reference(0), context: context(snippets))
    #expect(allowed.text == String(repeating: "x", count: 7) + "last")
    #expect(allowed.diagnostics.isEmpty)
    let repeated = TemplateRenderer.render(reference(7) + reference(7), context: context(snippets))
    #expect(repeated.text == "lastlast")
    #expect(repeated.diagnostics.isEmpty)
}

@Test func typedAttributesAndCanonicalization() {
    let parsed = TemplateParser.parse(#"{{ field name = x custom=42 trim=false default="a\"b\\c" }}"#)
    guard case .tag(let tag) = parsed.nodes.first else { Issue.record("Expected tag"); return }
    #expect(tag.attributes[1].value == .number(42))
    #expect(tag.attributes[2].value == .bool(false))
    #expect(tag.typed == .field(name: "x", default: "a\"b\\c", multiline: false))
    let canonical = TemplateSerializer.serialize(parsed.nodes)
    let reparsed = TemplateParser.parse(canonical)
    guard case .tag(let again) = reparsed.nodes.first else { Issue.record("Expected tag"); return }
    #expect(tag.name == again.name && tag.attributes == again.attributes)
}

@Test func invalidTypesAndDuplicateLastWins() {
    let result = TemplateRenderer.render("{{clipboard trim=oops}}", context: context())
    #expect(result.text == "{{clipboard trim=oops}}")
    #expect(result.diagnostics.count == 1)
    let duplicate = TemplateRenderer.render(#"{{field name=x default="first" default="last"}}"#, context: context())
    #expect(duplicate.text == "last")
    #expect(duplicate.diagnostics.count == 1)
}

@Test(arguments: ["3", "true", "\"nonsense\""])
func invalidOffsetIsIgnored(value: String) {
    let result = TemplateRenderer.render("{{date offset=\(value)}}", context: context())
    #expect(result.text == "2024-01-31")
    #expect(result.diagnostics.count == 1)
}

@Test func noncanonicalGeneratedRoundTrips() {
    for i in 0..<250 {
        let source = "😀\\{{literal}} {{ field   name = x custom = \(i).5 default=\"value \(i)\" }} {{clipboard trim = \(i % 2 == 0)}}"
        let first = TemplateParser.parse(source)
        let canonical = TemplateSerializer.serialize(first.nodes)
        let second = TemplateParser.parse(canonical)
        #expect(first.diagnostics.isEmpty && second.diagnostics.isEmpty)
        #expect(first.nodes.count == second.nodes.count)
        for (a, b) in zip(first.nodes, second.nodes) {
            switch (a, b) {
            case (.text(let x, _), .text(let y, _)): #expect(x == y)
            case (.tag(let x), .tag(let y)): #expect(x.name == y.name && x.attributes == y.attributes)
            default: Issue.record("Node kind changed")
            }
        }
        #expect(TemplateSerializer.serialize(second.nodes) == canonical)
    }
}

@Test func recursiveClipboardAndFormDiagnostics() {
    let rootID = UUID()
    let childID = UUID()
    let leafID = UUID()
    let snippets: [UUID: String] = [
        rootID: #"{{snippet id="\#(childID.uuidString)"}} {{field name=role multiline=true}}"#,
        childID: #"{{field name=role}} {{choice name=empty options=""}} {{snippet id="\#(leafID.uuidString)"}} {{snippet id="\#(rootID.uuidString)"}}"#,
        leafID: #"{{clipboard trim=true}}"#,
    ]
    let resolve: (UUID) -> String? = { snippets[$0] }

    #expect(TemplateAnalysis.usesClipboard(in: snippets[rootID]!, rootSnippetID: rootID, resolveSnippet: resolve))
    #expect(!TemplateAnalysis.usesClipboard(in: "{{field name=only}}", rootSnippetID: rootID, resolveSnippet: resolve))

    let fields = TemplateAnalysis.formFields(in: snippets[rootID]!, rootSnippetID: rootID, resolveSnippet: resolve)
    #expect(fields.map(\.name) == ["role", "empty"])
    #expect(fields.first?.kind == .text)

    let diagnostics = TemplateAnalysis.formDiagnostics(in: snippets[rootID]!, rootSnippetID: rootID, resolveSnippet: resolve)
    #expect(diagnostics.count == 2)
    #expect(diagnostics.contains { $0.message.contains("Dropdown 'empty' has no options") })
    #expect(diagnostics.contains { $0.message.contains("'role' is already a text field") })
}
