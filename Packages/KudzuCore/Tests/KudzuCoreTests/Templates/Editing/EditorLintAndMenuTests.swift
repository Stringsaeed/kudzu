import Foundation
import Testing
@testable import KudzuCore

@Test func literalDiagnosticsMapToDisplayAndSource() {
    // Hand-typed text; a "{{DATE}}" loaded from source would be a verbatim chip instead.
    let segments: [TemplateSegment] = [.text("a\\"), .chip(KudzuCore.Tag(name: "cursor")), .text("x {{DATE}}")]
    // Display: "a\" (2) + chip (1) + "x {{DATE}}"; source escapes the backslash and the literal "{{".
    let diagnostics = TemplateEditorLint.diagnostics(for: segments)
    #expect(diagnostics.count == 1)
    let diagnostic = diagnostics[0]
    #expect(diagnostic.displayRange == 5..<13)
    let source = TemplateSegments.serialize(segments)
    #expect(source == "a\\\\{{cursor}}x \\{{DATE}}")
    let units = Array(source.utf16)
    let slice = String(decoding: units[diagnostic.sourceRange], as: UTF16.self)
    #expect(slice == "\\{{DATE}}")
    #expect(diagnostic.sourceDiagnostic.range == diagnostic.sourceRange)
}

@Test func literalLintKeepsUnderliningAfterCanonicalization() {
    let typed: [TemplateSegment] = [.text("{{nope}}")]
    let first = TemplateEditorLint.diagnostics(for: typed)
    let saved = TemplateSegments.serialize(typed)
    #expect(saved == "\\{{nope}}")
    let second = TemplateEditorLint.diagnostics(for: TemplateSegments.parse(saved))
    #expect(first.map(\.displayRange) == [0..<8])
    #expect(second.map(\.displayRange) == first.map(\.displayRange))
}

@Test func chipDiagnostics() {
    let id = UUID(), current = UUID()
    let source = "{{cursor}}{{cursor}}{{field name=x}}{{choice name=x options=a}}{{date offset=zz}}"
        + "{{snippet id=\"\(id)\"}}{{snippet id=\"\(current)\"}}{{choice name=y}}"
    let diagnostics = TemplateEditorLint.diagnostics(for: TemplateSegments.parse(source), knownSnippetIDs: [current],
                                                     currentSnippetID: current)
    #expect(diagnostics.allSatisfy { $0.isChip })
    #expect(diagnostics.map(\.displayRange.lowerBound) == [1, 3, 4, 5, 6, 7])
    #expect(diagnostics.map(\.message) == [
        "Only the first cursor counts", "'x' is already a text field; the first definition wins",
        "Invalid date offset; ignored", "Missing snippet", "A snippet can't include itself", "Dropdown 'y' has no options",
    ])
}

@Test func sameKindFieldNamesAreFine() {
    let segments = TemplateSegments.parse("{{field name=x}}{{field name=x default=\"d\"}}")
    #expect(TemplateEditorLint.diagnostics(for: segments).isEmpty)
    #expect(!TemplateEditorLint.nameConflicts("x", kind: .text, in: segments))
    #expect(TemplateEditorLint.nameConflicts("x", kind: .choice, in: segments))
    #expect(!TemplateEditorLint.nameConflicts("x", kind: .choice, in: Array(segments.prefix(1)), excludingChipAt: 0))
}

@Test(arguments: [
    ("da", [TemplateInsertItem.date]), ("d", [.date, .choice]), ("", TemplateInsertItem.allCases),
    ("tex", [.field]), ("snip", [.snippet]), ("drop", [.choice]), ("zzz", []), ("CLIP", [.clipboard]),
])
func menuFiltering(query: String, expected: [TemplateInsertItem]) {
    #expect(TemplateInsertItem.matching(query) == expected)
}

@Test func insertedTagsAreValidAndUnique() {
    var segments = [TemplateSegment]()
    for item in TemplateInsertItem.allCases {
        let tag = item.makeTag(existing: segments)
        #expect(TemplateParser.parse(tag.source).diagnostics.isEmpty, "\(item)")
        #expect(tag.typed != .unknown)
        segments.append(.chip(tag))
    }
    #expect(TemplateInsertItem.field.makeTag(existing: segments).stringAttribute("name") == "field2")
    #expect(TemplateInsertItem.choice.makeTag(existing: segments).stringAttribute("name") == "choice2")
    #expect(TemplateInsertItem.snippet.makeTag().typed == .snippet(id: TemplateInsertItem.placeholderSnippetID))
    #expect(TemplateInsertItem.allCases.filter(\.editsOnInsert) == [.field, .choice, .snippet])
}

@Test(arguments: [
    ("/", 0, true), ("a /", 2, true), ("a\n/", 2, true), ("a/", 1, false), ("\u{FFFC}/", 1, false), ("x", 0, false),
])
func slashOpening(text: String, offset: Int, expected: Bool) {
    #expect(SlashCommand.canOpen(in: text, slashAt: offset) == expected)
}

@Test func slashQuery() {
    #expect(SlashCommand.query(in: "Hi /da", slashAt: 3, caret: 6) == "da")
    #expect(SlashCommand.query(in: "Hi /", slashAt: 3, caret: 4) == "")
    #expect(SlashCommand.query(in: "Hi /d a", slashAt: 3, caret: 7) == nil)
    #expect(SlashCommand.query(in: "Hi /da", slashAt: 3, caret: 3) == nil)
    #expect(SlashCommand.query(in: "Hi xda", slashAt: 3, caret: 6) == nil)
    #expect(SlashCommand.query(in: "/" + String(repeating: "a", count: 30), slashAt: 0, caret: 31) == nil)
}
