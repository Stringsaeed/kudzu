import Foundation
import Testing
@testable import KudzuCore

// Where kudzu-core uses `String` equality or `HashMap<String, _>` keys, names are equal only when their
// bytes are: `é` (U+00E9) and `e` + U+0301 are different form fields. Swift `String ==` would merge them.
// Trigger matching and conflict detection compare in NFC on purpose, and stay canonical.

private let composed = "\u{e9}", decomposed = "e\u{301}"
private let source = "{{field name=\"\(composed)\" default=A}}/{{field name=\"\(decomposed)\" default=B}}"

@Test func analysisKeepsCanonicallyEquivalentFieldNamesApart() {
    let fields = Template.analyze(Template.parse(source)).formFields
    #expect(fields.map(\.name.exactKey) == [composed.exactKey, decomposed.exactKey])
    #expect(Template.formFields(source, resolveSnippet: { _ in nil }).count == 2)
    #expect(Template.formFields("{{field name=x}}{{field name=x default=y}}", resolveSnippet: { _ in nil }).count == 1)
}

@Test func renderLooksUpFormValuesByBytes() {
    let now = Date(timeIntervalSince1970: 0)
    #expect(Template.render(source, context: RenderContext(now: now)).text == "A/B")
    // A value submitted for one spelling does not fill the other, as with Rust's HashMap.
    #expect(Template.render(source, context: RenderContext(now: now, formValues: [composed: "X"])).text == "X/B")
    #expect(Template.render(source, context: RenderContext(now: now, formValues: [decomposed: "Y"])).text == "A/Y")
}

@Test func formMemoryComparesNamesAndOptionsByBytes() throws {
    let id = UUID()
    var memory = FormMemory()
    memory.record([composed: "remembered", "pick": decomposed], for: id)
    let fields = [
        FormFieldSpec(name: decomposed, kind: .text, options: [], defaultValue: "default"),
        FormFieldSpec(name: "pick", kind: .choice, options: ["a", composed], defaultValue: nil),
    ]
    let values = memory.initialValues(for: id, fields: fields)
    // The remembered value belongs to the composed name; the remembered choice is not a listed option.
    #expect(values[decomposed] == "default")
    #expect(values["pick"] == "a")
}

@Test func editorFactsKeepFieldNamesApart() {
    #expect(Template.analyze(source).fieldNames.count == 2)
}

@Test func searchExactTriggerComparesCodeUnits() {
    let snippet = Snippet(name: "Other", triggers: [composed])
    let hit = SearchIndex(snippets: [snippet]).search(decomposed).first
    #expect((hit?.score ?? 0) < 10_000)
    #expect((SearchIndex(snippets: [snippet]).search(composed).first?.score ?? 0) >= 10_000)
}

@Test func triggerMatchingStaysCanonical() {
    // kudzu-core keys its tries by NFC clusters, so either spelling of a trigger fires.
    let snippet = Snippet(name: "s", triggers: ["caf\(composed)"], content: "x")
    var matcher = ExpansionMatcher(index: TriggerIndex(snippets: [snippet], groups: [], config: MatchingConfig()))
    var event: MatcherEvent?
    for key in ["c", "a", "f", "e", "\u{301}"] { event = matcher.handle(.character(key), bundleID: nil) }
    guard case .expand(let match) = event else { Issue.record("no expansion"); return }
    #expect(match.typedTrigger.isExactly("caf\(decomposed)"))
    // And two spellings on one snippet are one trigger; on two snippets they are duplicates.
    #expect(TriggerIndex(snippets: [Snippet(name: "d", triggers: [composed, decomposed])], groups: [], config: MatchingConfig()).triggerCount == 1)
    let pair = [Snippet(name: "a", triggers: [composed]), Snippet(name: "b", triggers: [decomposed])]
    #expect(findConflicts(snippets: pair, groups: []).map(\.kind) == [.duplicate])
}

@Test func triggerValidatorComparesBytesWhenCaseSensitive() {
    #expect((try? validateTrigger(decomposed, existing: [composed], caseSensitive: true).get())?.isExactly(decomposed) == true)
    #expect(validateTrigger(composed, existing: [composed], caseSensitive: true) == .failure(.duplicate))
}
