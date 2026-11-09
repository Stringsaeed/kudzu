import Foundation
import Testing
@testable import KudzuCore

private let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAJUlEQVR4nGMICLj0n5aYYdSCUQtGLRi1YNSCUQtGLRi1YGhYAACpwH6M7RqhEQAAAABJRU5ErkJggg=="
private func bold(_ start: Int, _ end: Int) -> FormatSpan { FormatSpan(start: start, end: end, style: .bold) }
@Test func oldLibrariesRemainPlainAndFormattingRoundTrips() throws {
    var snippet = try JSONDecoder().decode(Snippet.self, from: Data(#"{"id":"00000000-0000-0000-0000-000000000001","content":"Hello"}"#.utf8))
    #expect(snippet.formatting == nil)
    snippet.formatting = Formatting(source: "Hello", spans: [bold(0,5)])
    let restored = try JSONDecoder().decode(Snippet.self, from: JSONEncoder().encode(snippet))
    #expect(restored.formatting == snippet.formatting && restored.content == "Hello")
}
@Test func formattingTracksTemplatesEscapesUnicodeAndTheCursor() {
    let source = #"😀 \{{literal}} {{field name=who}}{{cursor}}!"#
    let start = source.distance(from: source.startIndex, to: source.range(of: "{{field")!.lowerBound) + 1
    let formatting = Formatting(source: source, spans: [bold(start, start + "{{field name=who}}".utf16.count)])
    let result = Template.renderFormatted(source, formatting: formatting, context: RenderContext(now: .now, formValues: ["who":"<Sam & سلمى>"]))
    #expect(result.text == "😀 {{literal}} <Sam & سلمى>!")
    #expect(result.cursorOffset == 27 && result.formatting == [bold(15,27)])
    #expect(Formatting.html(result.text, spans: result.formatting!).contains("<strong>&lt;Sam &amp; سلمى&gt;</strong>!"))
}
@Test func staleFormattingDoesNotStyleEditsFromAnOlderMac() {
    let result = Template.renderFormatted("after", formatting: Formatting(source:"before", spans:[bold(0,6)]), context: RenderContext(now:.now))
    #expect(result.text == "after" && result.formatting == nil)
}
@Test func htmlHandlesOverlapsAndRejectsActiveLinksAndInvalidRanges() {
    #expect(Formatting.html("abc", spans:[bold(0,2), FormatSpan(start:1,end:3,style:.italic)]).contains("<strong>a</strong><strong><em>b</em></strong><em>c</em>"))
    for url in ["javascript:alert(1)","data:text/html,bad","https://ok\" onmouseover=\"bad"] {
        #expect(!Formatting.html("safe", spans:[FormatSpan(start:0,end:4,style:.link(url:url))]).contains("<a "))
    }
    #expect(!Formatting.html("😀",spans:[bold(1,999)]).contains("<strong>"))
    #expect(Formatting.html("Site",spans:[FormatSpan(start:0,end:4,style:.link(url:"https://example.com/?a=1&b=2"))]).contains("href=\"https://example.com/?a=1&amp;b=2\""))
}
@Test func aFormattedReplyPreservesItsNestedSignatureButAPlainReplyDoesNot() {
    let id = UUID(), source = "Hello\n{{snippet id=\"\(UUID())\"}}"
    let signature = Formatting(source:"[Logo]",spans:[FormatSpan(start:0,end:6,style:.image(dataUrl:png,width:120))])
    let ctx = RenderContext(now:.now,resolveSnippet:{ _ in "[Logo]" },resolveFormatting:{ _ in signature },rootSnippetID:id)
    let rich = Template.renderFormatted(source,formatting:Formatting(source:source,spans:[]),context:ctx)
    #expect(rich.text == "Hello\n[Logo]" && rich.formatting?.first?.start == 6 && rich.formatting?.first?.end == 12)
    #expect(Formatting.html(rich.text,spans:rich.formatting!).contains(png))
    #expect(Template.render(source,context:ctx).formatting == nil)
}
@Test func embeddedImagesRoundTripWithAPlainTextAlternative() throws {
    let image = FormatSpan(start:3,end:9,style:.image(dataUrl:png,width:120))
    let snippet = Snippet(name:"Signature",triggers:[";sig"],content:"Hi [Logo]!",formatting:Formatting(source:"Hi [Logo]!",spans:[image]))
    let copy = try JSONDecoder().decode(Snippet.self,from:JSONEncoder().encode(snippet))
    #expect(copy.formatting == snippet.formatting && copy.content == "Hi [Logo]!")
    #expect(Formatting.html(copy.content,spans:[image]).contains("width=\"120\" alt=\"[Logo]\""))
    #expect(Formatting.insertionText(copy.content,spans:[image]) == "Hi \u{fffc}!")
    #expect(Formatting.insertionOffset(9,spans:[image]) == 4)
    #expect(!Formatting.safeImage("https://example.com/image.png") && !Formatting.safeImage("data:image/svg+xml;base64,PHN2Zz4="))
}
@Test func spanEditsRespectTextAndImageAffinity() {
    let image = FormatSpan(start:1,end:4,style:.image(dataUrl:png,width:120)), mark = bold(1,4)
    let edit = FormattingEdit(range:1..<1,insert:"X")
    #expect(EditorFormatting.mapSpans([image,mark],changes:[edit]) == [FormatSpan(start:2,end:5,style:image.style),bold(1,5)])
    #expect(EditorFormatting.changeFormat([bold(0,10)],range:3..<7,style:.bold) == [bold(0,3),bold(7,10)])
    #expect(EditorFormatting.selectedRange(3..<5,source:"x{{cursor}}y",spans:nil) == 1..<11)
}
@Test func distinctFormattingIsNotDeduplicatedAndRemovalSyncs() throws {
    let plain = Snippet(name:"Plain",triggers:[";x"],content:"Hello")
    let rich = Snippet(name:"Rich",triggers:[";x"],content:"Hello",formatting:Formatting(source:"Hello",spans:[bold(0,5)]))
    var library = Library(snippets:[plain,rich])
    #expect(Dedupe.plan(library).snippetMerges.isEmpty)
    let name = Sync.snippetName(rich.id); var index = Sync.SyncIndex()
    let record = try #require(Sync.encode(name,library:library,index:index,deviceID:"test"))
    Sync.recordSaved(index:&index,record:record)
    library.snippets[1].formatting = nil
    let updated = try #require(Sync.encode(name,library:library,index:index,deviceID:"test"))
    #expect(!updated.payload.contains("formatting"))
}
@Test func canonicallyEquivalentEditsInvalidateSourceFormatting() {
    let source = "é", older = "e\u{301}"
    #expect(Formatting.sourceSpans(source,formatting:Formatting(source:older,spans:[bold(0,2)])) == nil)
    #expect(Template.renderFormatted(source,formatting:Formatting(source:older,spans:[bold(0,2)]),context:RenderContext(now:.now)).formatting == nil)
    #expect(!Formatting.sameStyle(.link(url:"https://x/é"),.link(url:"https://x/e\u{301}")))
}
@Test func listPrefixesRoundTripAndImageEdgeEditsStayOutside() {
    func apply(_ edits: [FormattingEdit], to text: String) -> String {
        var units = Array(text.utf16)
        for e in edits.reversed() { units.replaceSubrange(e.range,with:e.insert.utf16) }
        return String(decoding:units,as:UTF16.self)
    }
    let listed = apply(EditorFormatting.toggleList("One\nTwo",selection:0..<7,ordered:true),to:"One\nTwo")
    #expect(listed == "1. One\n2. Two")
    #expect(apply(EditorFormatting.toggleList(listed,selection:0..<listed.utf16.count,ordered:true),to:listed) == "One\nTwo")
    let image = FormatSpan(start:0,end:6,style:.image(dataUrl:png,width:120))
    #expect(EditorFormatting.mapSpans([image],changes:[FormattingEdit(range:6..<6,insert:"!")]) == [image])
    #expect(EditorFormatting.mapSpans([image],changes:[FormattingEdit(range:0..<6,insert:"")]) == [])
    #expect(EditorFormatting.mapSpans([bold(0,5)],changes:[FormattingEdit(range:0..<5,insert:"Hi")]) == [bold(0,2)])
}
