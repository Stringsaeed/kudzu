import KudzuCore
import Testing
@testable import KudzuEditor

@Suite struct CoreParityTests {
    @Test func editorKeepsTheWebLinkValidationBoundary() throws {
        #expect(["https://a/\u{FEFF}b", "https://a/\u{0085}b"].map(EditorLinkRules.safeLink) == [false, true])
        let state = EditorState(source: "Link", spans: [], selection: 0..<4)
        let linked = try #require(state.changingFormat(.link(url: "https://a/\u{0085}b")))
        #expect(linked.spans == [FormatSpan(start: 0, end: 4, style: .link(url: "https://a/\u{0085}b"))])
        #expect(FormattingHelpers.validSpans(linked.source, linked.spans ?? []).isEmpty)
        #expect(EditorLinkRules.safeLink("HTTPS://example.com"))
        #expect(EditorLinkRules.safeLink("MAILTO:me@example.com"))
        #expect(!EditorLinkRules.safeLink("https:/example.com"))
        #expect(!EditorLinkRules.safeLink("https://a/ b"))
        #expect(!EditorLinkRules.safeLink("javascript:alert(1)"))
    }

    @Test func linkWhitespaceFollowsRustLibraryContract() {
        // JavaScript `\s` rejects U+FEFF and accepts U+0085. KudzuCore follows Rust
        // `is_whitespace` / `is_control` (and template-formatting.json): BOM is safe, NEL is not.
        // The editor link field calls this same function, so it must not expect the JS pair.
        let actual = ["https://a/\u{FEFF}b", "https://a/\u{0085}b"].map(FormattingHelpers.safeLink)
        #expect(actual == [true, false])
    }
}
