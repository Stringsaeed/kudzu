import Foundation
import Testing
@testable import KudzuCore

// search.ts and the palette's `#group` filter, at the edges where Swift and JavaScript strings differ.

@Test func displayNameSkipsAnEmptyFirstTrigger() {
    let snippet = Snippet(name: "", triggers: ["", "x"])
    #expect(SearchIndex(snippets: [snippet]).entries.first?.name == "Untitled")
    #expect(SearchIndex.buildIndex([snippet]).first?.name == "Untitled")
}

@Test func queryTrimsAndSplitsOnJavaScriptWhitespace() {
    let snippet = Snippet(name: "Other", triggers: ["sig"])
    let index = SearchIndex(snippets: [snippet])
    #expect(index.search("\u{feff}sig\u{feff}").first?.field == .trigger)
    #expect((index.search("\u{feff}sig\u{feff}").first?.score ?? 0) >= 10_000)
}

@Test func groupPrefixComparesCodeUnits() {
    // JavaScript `startsWith` sees the `e` of a decomposed `é`.
    let group = Group(name: "E\u{301}cole")
    let a = Snippet(name: "Invoice", groupID: group.id)
    #expect(SearchIndex(snippets: [a]).search("#e invoice", groups: [group]).map(\.snippet.id) == [a.id])
}
