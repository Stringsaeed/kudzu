import Foundation
import Testing
@testable import KudzuCore

// Replays `template-formatting.json` (export_fixtures.rs, template section): `formatting.rs`'s HTML,
// attachment text and offsets, and URL safety, including quotes and brackets followed by combining marks.

private struct FormattingGoldenFile: Decodable {
    struct Case: Decodable {
        var text: String
        var spans: [FormatSpan]
        var html: String
        var insertionText: String
        var insertionOffsets: [Int]
    }
    struct Verdict: Decodable { var url: String; var safe: Bool }
    var cases: [Case]
    var links: [Verdict]
    var images: [Verdict]
}

@Test func formattingGoldenFixtures() throws {
    let file = try loadT1Fixture("template-formatting.json", as: FormattingGoldenFile.self)
    for c in file.cases {
        #expect(Formatting.html(c.text, spans: c.spans).utf8.elementsEqual(c.html.utf8), "html: \(c.text.debugDescription)")
        #expect(Formatting.insertionText(c.text, spans: c.spans).utf8.elementsEqual(c.insertionText.utf8), "insertionText: \(c.text.debugDescription)")
        #expect(c.insertionOffsets.indices.map { Formatting.insertionOffset($0, spans: c.spans) } == c.insertionOffsets, "insertionOffset: \(c.text.debugDescription)")
    }
    for link in file.links { #expect(Formatting.safeLink(link.url) == link.safe, "safeLink: \(link.url.debugDescription)") }
    for image in file.images { #expect(Formatting.safeImage(image.url) == image.safe, "safeImage: \(image.url.prefix(60).debugDescription)") }
}

@Test func htmlEscapesQuotesFollowedByCombiningMarks() {
    // An image's alt text comes from snippet content; a quote carrying a combining mark must not end the attribute.
    let text = "[\"\u{301} onerror=x]"
    let spans = [FormatSpan(start: 0, end: text.utf16.count, style: .image(dataUrl: "data:image/png;base64,iVBORw0KGgo=", width: 8))]
    #expect(Formatting.html(text, spans: spans).contains("alt=\"[&quot;\u{301} onerror=x]\""))
    #expect(Formatting.escapeHTML("<\u{301}&\u{301}'\u{301}") == "&lt;\u{301}&amp;\u{301}&#39;\u{301}")
}

@Test func safeLinkPrefixIgnoresGraphemeBoundaries() {
    #expect(Formatting.safeLink("https://\u{301}x"))
    #expect(Formatting.safeLink("HTTPS://kudzu.site"))
    #expect(!Formatting.safeLink("https\u{130}://x"))
}

@Test func validSpansDefaultsToTheRustLinkRule() {
    let text = "Link"
    let bom = FormatSpan(start: 0, end: 4, style: .link(url: "https://a/\u{FEFF}b"))
    let nel = FormatSpan(start: 0, end: 4, style: .link(url: "https://a/\u{0085}b"))
    #expect(FormattingHelpers.validSpans(text, [bom, nel]) == [bom])
    #expect(FormattingHelpers.validSpans(text, [nel]) { _ in true } == [nel])
}
