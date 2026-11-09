import Foundation
import Testing
@testable import KudzuCore

// Behaviour of the TypeScript client helpers (`lib/*.ts`) and Swift-only paths the Rust fixtures do
// not reach. JavaScript's `\s` (used by template.ts and search.ts) includes U+FEFF but not U+0085.

@Test func icuFractionStaysInItsSecondWithoutExactNanoseconds() throws {
    // 2023-12-31T23:59:59.9999996Z: Chrono prints the true second; rounding must not reset it to `.000000`.
    let date = Date(timeIntervalSince1970: 1_704_067_199.9999996)
    #expect(Template.formatICU(date, pattern: "yyyy-MM-dd HH:mm:ss.SSSSSS", locale: "en_US") == "2023-12-31 23:59:59.999999")
    #expect(Template.formatICU(Date(timeIntervalSince1970: 1.5), pattern: "ss.SSS", locale: "en_US") == "01.500")
}

@Test func tokenizerUsesJavaScriptWhitespace() {
    func tag(_ source: String) -> TagToken? {
        guard case .tag(let tag)? = Template.tokenize(source).first else { return nil }
        return tag
    }
    #expect(tag("{{date\u{feff}format=\"x\"}}")?.verbatim == false)
    #expect(tag("{{date\u{feff}format=\"x\"}}")?.attrs["format"] == .string("x"))
    #expect(tag("{{date\u{85}format=\"x\"}}")?.verbatim == true)
    #expect(tag("{{date\u{3000}format=\"x\"}}")?.verbatim == false)
}

@Test func plainSummaryCollapsesJavaScriptWhitespace() {
    #expect(Template.plainSummary("a\u{feff}\u{feff}b ") == "a b")
    #expect(Template.plainSummary("a\u{85}b") == "a\u{85}b")
    #expect(Template.plainSummary(" \t{{cursor}}\n x ") == "‹Cursor› x")
}

@Test func joinOptionsEscapesEveryPipe() {
    let options = ["a|\u{301}b", "c"]
    #expect(Template.joinOptions(options) == "a\\|\u{301}b|c")
    #expect(Template.splitOptions(Template.joinOptions(options)) == options)
}

@Test func listMarkersAreASCIIDigits() {
    #expect(EditorFormatting.toggleList("١. x", selection: 0..<0, ordered: true) == [FormattingEdit(range: 0..<0, insert: "1. ")])
    #expect(EditorFormatting.toggleList("12. x", selection: 0..<0, ordered: true) == [FormattingEdit(range: 0..<4, insert: "")])
}

@Test func triggerTokensSplitOnWhitespaceScalars() {
    // Rust's `split_whitespace` keeps a combining mark that follows a space with the next token.
    #expect(triggerTokens(" \u{301}a\u{3000}b\r\nc ") == ["\u{301}a", "b", "c"])
}
