import Foundation
import Testing
@testable import KudzuCore

private func context(_ snippets: [UUID: String] = [:], root: UUID? = nil) -> RenderContext {
    RenderContext(now: Date(timeIntervalSince1970: 1706659200), calendar: Calendar(identifier: .gregorian),
                  timeZone: TimeZone(secondsFromGMT: 0)!, locale: Locale(identifier: "en_US_POSIX"),
                  clipboard: "clip", formValues: [:], resolveSnippet: { snippets[$0] }, rootSnippetID: root)
}

private func render(_ source: String) -> String { TemplateRenderer.render(source, context: context()).text }
private func canonical(_ source: String) -> String { TemplateSerializer.serialize(TemplateParser.parse(source).nodes) }

/// Nodes with ranges cleared, adjacent text merged and empty text dropped, for semantic comparison.
private func semantic(_ nodes: [TemplateNode]) -> [TemplateNode] {
    var result = [TemplateNode]()
    for node in nodes {
        switch node {
        case .text(let text, _):
            guard !text.isEmpty else { continue }
            if case .text(let previous, _)? = result.last { result[result.count - 1] = .text(previous + text, range: 0..<0) }
            else { result.append(.text(text, range: 0..<0)) }
        case .tag(var tag):
            tag.range = 0..<0
            result.append(.tag(tag))
        }
    }
    return result
}

// MARK: - Bare numbers

@Test func bareNumberIsAcceptedForStringAttributes() {
    let parsed = TemplateParser.parse("{{field name=qty default=5}}")
    #expect(parsed.diagnostics.isEmpty)
    guard case .tag(let tag) = parsed.nodes.first else { Issue.record("Expected a tag"); return }
    #expect(tag.typed == .field(name: "qty", default: "5", multiline: false))
    #expect(render("{{field name=qty default=5}}") == "5")
    #expect(canonical("{{field name=qty default=5}}") == #"{{field name="qty" default="5"}}"#)
}

@Test(arguments: ["5", "-3", "+7", "007", "1.50", "-0.25", "10", "true", "false"])
func bareTokensKeepTheirWrittenTextInStringAttributes(token: String) {
    #expect(render("{{field name=x default=\(token)}}") == token)
    #expect(render("{{choice name=x options=\"a|b\" default=\(token)}}") == token)
    #expect(TemplateParser.parse("{{field name=x default=\(token)}}").diagnostics.isEmpty)
}

@Test func bareNumbersWorkInEveryStringAttribute() {
    #expect(TemplateParser.parse("{{field name=7}}").diagnostics.isEmpty)
    #expect(TemplateParser.parse("{{choice name=2 options=1|2}}").diagnostics.isEmpty == false) // '|' is not a bare-token character
    #expect(TemplateParser.parse("{{choice name=2 options=12 default=1}}").diagnostics.isEmpty)
    #expect(TemplateParser.parse("{{date format=5}}").diagnostics.isEmpty)
    #expect(render("{{date format=5}}") == "5")
    #expect(TemplateParser.parse("{{date locale=5}}").diagnostics.isEmpty)
}

@Test func numbersStillCannotBeBooleansOrIDs() {
    #expect(render("{{clipboard trim=1}}") == "{{clipboard trim=1}}")
    #expect(render("{{field name=x multiline=0}}") == "{{field name=x multiline=0}}")
    #expect(render("{{snippet id=5}}") == "{{snippet id=5}}")
    #expect(!TemplateParser.parse("{{clipboard trim=1}}").diagnostics.isEmpty)
}

@Test func invalidOffsetNumbersStillWarnOnceAndAreIgnored() {
    let result = TemplateRenderer.render("{{date offset=3}}", context: context())
    #expect(result.text == "2024-01-31")
    #expect(result.diagnostics.count == 1)
}

@Test func unknownAttributesKeepTypedNumbers() {
    func attribute(_ source: String) -> AttributeValue? {
        guard case .tag(let tag) = TemplateParser.parse(source).nodes.first else { return nil }
        return tag.attributes.last?.value
    }
    #expect(attribute("{{cursor n=5}}") == .number(5))
    #expect(attribute("{{cursor n=-2.5}}") == .number(-2.5))
    #expect(attribute("{{cursor n=+3}}") == .number(3))
    // Only plain decimals are numbers; anything else bare is a string.
    for token in ["1e5", ".5", "5.", "0x10", "1_000", "5d", "--5", "+-5"] {
        #expect(attribute("{{cursor n=\(token)}}") == .string(token), "\(token)")
    }
}

@Test func numbersSerializeAsPlainDecimalsAndRoundTrip() {
    for value in [0.0, 42, -7, 2.5, -0.125, 1e20, 123_456_789_012_345_678, 1e-7, 0.1, 1e300] {
        let node = TemplateNode.tag(KudzuCore.Tag(name: "cursor", attributes: [Attribute(name: "n", value: .number(value))]))
        let source = TemplateSerializer.serialize([node])
        #expect(!source.contains("e"), "\(source)")
        let parsed = TemplateParser.parse(source)
        #expect(parsed.diagnostics.isEmpty)
        guard case .tag(let tag) = parsed.nodes.first, case .number(let again) = tag.attributes[0].value else {
            Issue.record("Not a number: \(source)"); continue
        }
        #expect(again == value, "\(source)")
    }
    #expect(canonical("{{cursor n=42}}") == "{{cursor n=42}}")
}

// MARK: - Literal brace before a tag

@Test(arguments: [
    ("{{{date}}", "{2024-01-31"),
    ("{{{date}}}", "{2024-01-31}"),
    ("{{{{date}}", "{{2024-01-31"),
    ("a{{{clipboard}}b", "a{clipb"),
    ("{{{", "{{{"),
    ("{{{x", "{{{x"),
    ("{{{{", "{{{{"),
])
func literalBraceBeforeTag(source: String, expected: String) {
    #expect(render(source) == expected)
}

@Test func braceBeforeTagParsesAsTextThenTag() {
    let nodes = semantic(TemplateParser.parse("{{{date}}").nodes)
    #expect(nodes == [.text("{", range: 0..<0), .tag(KudzuCore.Tag(name: "date"))])
    let ranges = TemplateParser.parse("é{{{date}}").nodes.map(\.range)
    #expect(ranges == [0..<2, 2..<10])
}

@Test func serializerEmitsRoundTrippingBraceEscapes() {
    let date = TemplateNode.tag(KudzuCore.Tag(name: "date"))
    func text(_ value: String) -> TemplateNode { .text(value, range: 0..<0) }
    #expect(TemplateSerializer.serialize([text("{"), date]) == "{{{date}}")
    #expect(TemplateSerializer.serialize([text("{{"), date]) == "\\{{{{date}}")
    #expect(TemplateSerializer.serialize([text("{{{"), date]) == "\\{{{{{date}}")
    #expect(TemplateSerializer.serialize([text("a{{{b")]) == "a\\{{{b")
    // Adjacent text is merged before escaping: "{" + "{" is "{{".
    #expect(TemplateSerializer.serialize([text("{"), text("{")]) == "\\{{")
    #expect(TemplateSerializer.serialize([text("{"), text("{"), date]) == "\\{{{{date}}")
    for nodes in [[text("{"), date], [text("{{"), date], [text("\\{"), date], [text("{{{"), date, text("{")]] {
        let source = TemplateSerializer.serialize(nodes)
        #expect(semantic(TemplateParser.parse(source).nodes) == semantic(nodes), "\(source)")
    }
}

/// Sources that were valid (no error diagnostics) before `{{{` gained a meaning must read exactly as before.
@Test(arguments: [
    (#"\{{"#, "{{"), (#"\{{cursor}}"#, "{{cursor}}"), (#"\\"#, #"\"#), (#"\\\\"#, #"\\"#),
    (#"\{x"#, #"\{x"#), (#"a\{"#, #"a\{"#), (#"\{ \}"#, #"\{ \}"#), (#"\}"#, #"\}"#), (#"\x"#, #"\x"#),
    (#"\\{{cursor}}"#, #"\"#), (#"\\\{{cursor}}"#, #"\{{cursor}}"#),
    (#"\{{{date}}"#, "{{{date}}"), (#"\{{{{date}}"#, "{{2024-01-31"),
    ("a{b", "a{b"), ("{ {{cursor}}", "{ "), ("{}", "{}"), ("}}{{cursor}}{", "}}{"),
    (#"{{field name="{{{"}}"#, ""), (#"{{field name="{{{" default="x"}}"#, "x"),
])
func existingValidSourcesKeepTheirMeaning(source: String, expected: String) {
    #expect(render(source) == expected)
}

@Test func serializerOutputMatchesTheLegacySerializerForOrdinaryText() {
    for text in ["", "abc", "{", "}", "a{b", "{{", "\\", "\\\\", "\\{{", "x\\{{y", "{{{", "{{{{", "😀{{é"] {
        let legacy = text.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "{{", with: "\\{{")
        #expect(TemplateSerializer.serialize([.text(text, range: 0..<0)]) == legacy, "\(text)")
    }
}

// MARK: - Unrecognized tags stay verbatim

private let unrecognized = [
    "{{DATE}}", "{{unknown}}", "{{ future  size = 3 mode=\"x\" }}", "{{field}}", "{{choice}}", "{{clipboard trim=oops}}",
    "{{snippet id=nope}}", "{{date format=}}", "{{ }}", "{{}}", "{{9}}", "{{a-b}}",
    "{{date format=\"y\" format=\"z\" bad}}", "{{field name=x multiline=\"yes\"}}",
]

@Test(arguments: unrecognized)
func unrecognizedTagsSurviveASaveVerbatim(tag: String) {
    for source in [tag, "a" + tag, tag + "b", "a" + tag + "{{cursor}}" + tag + "\\\\"] {
        let parsed = TemplateParser.parse(source)
        #expect(TemplateSerializer.serialize(parsed.nodes) == source, "\(source)")
        #expect(parsed.diagnostics.contains { $0.severity == .error })
    }
}

@Test(arguments: unrecognized)
func unrecognizedTagsRenderExactlyAsTheyAlwaysDid(tag: String) {
    #expect(render(tag) == tag)
    #expect(render("x" + tag + "{{cursor}}y") == "x" + tag + "y")
    // Rendering the source and rendering the re-saved source agree.
    #expect(render(canonical(tag)) == render(tag))
}

@Test func unrecognizedTagIsATagNodeWithoutRecognizedMeaning() {
    let parsed = TemplateParser.parse("é{{DATE}}")
    #expect(parsed.nodes.count == 2)
    guard case .tag(let tag) = parsed.nodes[1] else { Issue.record("Expected a tag node"); return }
    #expect(tag.verbatim == "{{DATE}}")
    #expect(tag.typed == .unknown)
    #expect(tag.range == 1..<9)
    #expect(parsed.diagnostics.map(\.range) == [1..<9])
}

@Test func escapedTagSyntaxIsStillEscapedText() {
    // The user asked for literal text; it must not become a tag on save.
    #expect(canonical(#"\{{DATE}}"#) == #"\{{DATE}}"#)
    #expect(canonical(#"\{{date}}"#) == #"\{{date}}"#)
    #expect(TemplateSerializer.serialize([.text("{{DATE}}", range: 0..<0)]) == #"\{{DATE}}"#)
    #expect(TemplateSerializer.serialize([.text("{{date}}", range: 0..<0)]) == #"\{{date}}"#)
}

@Test func unterminatedTagIsLiteralText() {
    let parsed = TemplateParser.parse("a{{date")
    #expect(semantic(parsed.nodes) == [.text("a{{date", range: 0..<0)])
    #expect(parsed.diagnostics.map(\.message) == ["Unterminated tag"])
    #expect(render("a{{date") == "a{{date")
    // An unclosed quote swallows the rest of the template, so it is unterminated too.
    let quoted = TemplateParser.parse(#"{{field name="oops}}"#)
    #expect(semantic(quoted.nodes) == [.text(#"{{field name="oops}}"#, range: 0..<0)])
    #expect(canonical(#"{{field name="oops}}"#) == #"\{{field name="oops}}"#)
}

@Test func analysisIgnoresUnrecognizedTags() {
    let info = TemplateAnalysis.analyze(TemplateParser.parse("{{field}}{{clipboard trim=oops}}{{DATE}}"))
    #expect(!info.isForm && !info.usesClipboard && !info.hasCursor)
}

@Test func chipEditorKeepsUnrecognizedTagsVerbatim() {
    let source = "Hi {{ Future  x=1 }}, {{field}} and {{date}}"
    let segments = TemplateSegments.parse(source)
    #expect(segments.count == 6)
    guard case .chip(let first) = segments[1] else { Issue.record("Expected a chip"); return }
    #expect(first.verbatim == "{{ Future  x=1 }}")
    // An edit elsewhere in the snippet leaves the unrecognized tags exactly as written.
    var edited = segments
    edited.insert(.text("!"), at: 0)
    #expect(TemplateSegments.serialize(edited) == "!Hi {{ Future  x=1 }}, {{field}} and {{date}}")
    #expect(TemplateSegments.serialize(segments) == source)
}

@Test func unrecognizedChipsAreNotCreatedFromTypingOrPasting() {
    #expect(TemplateSegments.typedTag(in: "{{unknown}}", endingAt: 11) == nil)
    #expect(TemplateSegments.typedTag(in: "{{field}}", endingAt: 9) == nil)
    #expect(TemplateSegments.pasted("{{DATE}}") == [.text("{{DATE}}")])
    #expect(TemplateSegments.serialize(TemplateSegments.pasted("{{DATE}}")) == #"\{{DATE}}"#)
}

@Test func longUnrecognizedChipTitlesAreShortenedToOneLine() {
    let long = KudzuCore.Tag(name: "", verbatim: "{{future text=\"" + String(repeating: "x", count: 80) + "\nmore\"}}")
    let title = ChipLabel(tag: long).title
    #expect(title.count == 40 && title.hasSuffix("…") && !title.contains("\n"))
    #expect(ChipLabel(tag: long).accessibilityLabel.contains("kept as written"))
}

@Test func unrecognizedChipsHaveAnUnknownLabelAndLintMessage() {
    let segments = TemplateSegments.parse("{{ Future  x=1 }}{{field}}")
    guard case .chip(let tag) = segments[0] else { Issue.record("Expected a chip"); return }
    let label = ChipLabel(tag: tag)
    #expect(label.kind == .unknown && label.isWarning)
    #expect(label.title == "{{ Future  x=1 }}")
    let lint = TemplateEditorLint.diagnostics(for: segments)
    #expect(lint.count == 2 && lint.allSatisfy(\.isChip))
    #expect(lint[0].message == "Malformed tag")
    #expect(lint[1].message == "field requires name")
    #expect(lint[0].displayRange == 0..<1 && lint[1].displayRange == 1..<2)
    #expect(lint[0].sourceRange == 0..<17 && lint[1].sourceRange == 17..<26)
}

// MARK: - Self reference through the root ID

@Test func rootSnippetCannotIncludeItselfEvenOnce() {
    let root = UUID()
    let tag = "{{snippet id=\"\(root)\"}}"
    let source = "before " + tag + " after"
    let withRoot = TemplateRenderer.render(source, context: context([root: source], root: root))
    #expect(withRoot.text == "before  after")
    #expect(withRoot.diagnostics.map(\.message) == ["Snippet cycle detected"])
    // Without the root ID the self reference expands once before the cycle is noticed.
    let without = TemplateRenderer.render(source, context: context([root: source]))
    #expect(without.text == "before before  after after")
}

@Test func rootIDDoesNotAffectOtherReferences() {
    let root = UUID(), other = UUID()
    let result = TemplateRenderer.render("{{snippet id=\"\(other)\"}}", context: context([other: "x"], root: root))
    #expect(result.text == "x")
    #expect(result.diagnostics.isEmpty)
    // The root can still be reached through another snippet only once it is on the path, which is a cycle.
    let indirect = TemplateRenderer.render("{{snippet id=\"\(other)\"}}", context: context([other: "{{snippet id=\"\(root)\"}}", root: "never"], root: root))
    #expect(indirect.text == "")
    #expect(indirect.diagnostics.count == 1)
}

@Test func renderContextRootDefaultsToNil() {
    #expect(context().rootSnippetID == nil)
    let plain = RenderContext(now: .now, calendar: .current, timeZone: .current, locale: .current)
    #expect(plain.rootSnippetID == nil)
}

// MARK: - Property style round trips

private struct SplitMix64: RandomNumberGenerator {
    var state: UInt64
    mutating func next() -> UInt64 {
        state &+= 0x9E37_79B9_7F4A_7C15
        var z = state
        z = (z ^ (z >> 30)) &* 0xBF58_476D_1CE4_E5B9
        z = (z ^ (z >> 27)) &* 0x94D0_49BB_1331_11EB
        return z ^ (z >> 31)
    }
}

private func randomText(_ rng: inout SplitMix64, alphabet: [String] = ["a", "{", "}", "\\", "\"", " ", "😀", "\n", "|", "{{", "}}", "\\{", "\\\\"]) -> String {
    (0..<Int.random(in: 0...6, using: &rng)).map { _ in alphabet.randomElement(using: &rng)! }.joined()
}

private func randomTag(_ rng: inout SplitMix64) -> KudzuCore.Tag {
    let string = { (rng: inout SplitMix64) in AttributeValue.string(randomText(&rng)) }
    switch Int.random(in: 0..<8, using: &rng) {
    case 0: return KudzuCore.Tag(name: "date", attributes: [Attribute(name: "format", value: string(&rng))])
    case 1: return KudzuCore.Tag(name: "cursor")
    case 2: return KudzuCore.Tag(name: "clipboard", attributes: [Attribute(name: "trim", value: .bool(Bool.random(using: &rng)))])
    case 3: return KudzuCore.Tag(name: "field", attributes: [Attribute(name: "name", value: string(&rng)), Attribute(name: "default", value: string(&rng))])
    case 4: return KudzuCore.Tag(name: "choice", attributes: [Attribute(name: "name", value: .string("c")), Attribute(name: "options", value: string(&rng))])
    case 5: return KudzuCore.Tag(name: "snippet", attributes: [Attribute(name: "id", value: .string(UUID().uuidString))])
    case 6: return KudzuCore.Tag(name: "time", attributes: [Attribute(name: "extra", value: .number(Double(Int.random(in: -999...999, using: &rng))))])
    default:
        return KudzuCore.Tag(name: "", verbatim: ["{{DATE}}", "{{ nope }}", "{{field}}", "{{x y=1}}"].randomElement(using: &rng)!)
    }
}

@Test func generatedNodesRoundTripThroughTheSerializer() {
    var rng = SplitMix64(state: 20260413)
    for _ in 0..<4000 {
        var nodes = [TemplateNode]()
        for _ in 0..<Int.random(in: 0...6, using: &rng) {
            nodes.append(Bool.random(using: &rng) ? .text(randomText(&rng), range: 0..<0) : .tag(randomTag(&rng)))
        }
        let source = TemplateSerializer.serialize(nodes)
        let parsed = TemplateParser.parse(source)
        #expect(semantic(parsed.nodes) == semantic(nodes), "source: \(source)")
        #expect(TemplateSerializer.serialize(parsed.nodes) == source, "source: \(source)")
        #expect(!parsed.diagnostics.contains { $0.severity == .error && $0.message == "Unterminated tag" }, "source: \(source)")
    }
}

@Test func arbitrarySourceIsStableAfterOneSave() {
    var rng = SplitMix64(state: 7)
    let pieces = ["a", "{", "{{", "{{{", "}", "}}", "\\", "\\\\", "\\{{", "\\{", "\"", " ", "=", "date", "x=1", "{{date}}",
                  "{{cursor}}", "{{DATE}}", "{{field name=q default=5}}", "{{clipboard trim=true}}", "😀", "|", "{{unknown a=\"b\"}}"]
    for _ in 0..<6000 {
        let source = (0..<Int.random(in: 0...9, using: &rng)).map { _ in pieces.randomElement(using: &rng)! }.joined()
        let first = TemplateParser.parse(source)
        let saved = TemplateSerializer.serialize(first.nodes)
        let second = TemplateParser.parse(saved)
        #expect(semantic(second.nodes) == semantic(first.nodes), "source: \(source) saved: \(saved)")
        #expect(TemplateSerializer.serialize(second.nodes) == saved, "source: \(source)")
        // Saving never changes what the template renders.
        #expect(render(saved) == render(source), "source: \(source) saved: \(saved)")
        // Editor segments agree with the serializer.
        #expect(TemplateSegments.serialize(TemplateSegments.parse(source)) == saved, "source: \(source)")
        #expect(TemplateSegments.parse(saved) == TemplateSegments.parse(source), "source: \(source)")
    }
}

@Test func braceAndBackslashRunsNextToTagsRoundTrip() {
    for braces in 0...5 {
        for slashes in 0...5 {
            for tail in 0...3 {
                let text = String(repeating: "\\", count: slashes) + String(repeating: "{", count: braces)
                let nodes: [TemplateNode] = [.text(text, range: 0..<0), .tag(KudzuCore.Tag(name: "cursor")), .text(String(repeating: "}", count: tail), range: 0..<0), .tag(KudzuCore.Tag(name: "clipboard"))]
                let source = TemplateSerializer.serialize(nodes)
                #expect(semantic(TemplateParser.parse(source).nodes) == semantic(nodes), "source: \(source)")
                let result = TemplateRenderer.render(source, context: context())
                #expect(result.text == text + String(repeating: "}", count: tail) + "clip", "source: \(source)")
                #expect(result.cursorOffset == text.utf16.count)
            }
        }
    }
}
