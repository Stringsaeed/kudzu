import Foundation
import Testing
@testable import KudzuCore

@Test func exactTriggerWinsAndPrefixesBeatNameMatches() {
    let exact = Snippet(name:"Other",triggers:["sig"]), prefix = Snippet(name:"Unrelated",triggers:["signature"]), name = Snippet(name:"Signature")
    let index = SearchIndex(snippets:[name,prefix,exact])
    #expect(index.search("sig").map(\.snippet.id) == [exact.id,prefix.id,name.id])
    #expect(index.search("sig")[0].field == .trigger)
}
@Test func searchTightensSubsequencesAndHighlightsUTF16Positions() {
    let result = SearchIndex.fuzzy("ab", "a--a-b")
    #expect(result?.positions == [3,5])
    let snippet = Snippet(name:"😀 alpha beta")
    let hits = SearchIndex(snippets:[snippet]).search("ab")
    #expect(hits.first?.nameHits == [7,9])
    #expect(SearchIndex(snippets:[snippet]).search("zz").isEmpty)
}
@Test func termsCanMatchDifferentFieldsAndContentIsContiguous() {
    let snippet = Snippet(name:"Receipt",triggers:[";inv"],content:"invoice total amount")
    let index = SearchIndex(snippets:[snippet])
    #expect(index.search("rec amount").count == 1)
    #expect(index.search("rec amnt").isEmpty)
    #expect(index.search("i").count == 1)
}
@Test func emptyQueryUsesUsageAndRecentActivity() {
    let a = Snippet(name:"A"), b = Snippet(name:"B"), now = Date(timeIntervalSince1970: 200000)
    let index = SearchIndex(snippets:[a,b],usage:[b.id:SnippetUsage(count:3,last:now.timeIntervalSince1970 * 1000)])
    #expect(index.search("",now:now).first?.snippet.id == b.id)
    #expect(index.search("",limit:1).count == 1)
}
@Test func groupTermsFilterDirectMembershipByPrefix() {
    let group = Group(name:"Work"), other = Group(name:"Personal")
    let a = Snippet(name:"Invoice",groupID:group.id), b = Snippet(name:"Invoice",groupID:other.id)
    let index = SearchIndex(snippets:[b,a])
    #expect(index.search("#wo invoice",groups:[group,other]).map(\.snippet.id) == [a.id])
    #expect(index.search("#missing",groups:[group,other]).isEmpty)
}
@Test func chipLabelsAndTokenizerMatchClientHelpers() throws {
    let tags = Template.tokenize(#"😀 {{date}} {{field name=007 default=5}} {{clipboard trim=true}} {{future n=3}}"#).compactMap { token -> TagToken? in if case .tag(let t) = token { t } else { nil } }
    #expect(tags[0].from == 3 && Template.chipLabel(tags[0]) == "Date · ISO date")
    #expect(tags[1].attrs["name"] == .string("007") && Template.chipTone(tags[1]) == .form)
    #expect(Template.chipLabel(tags[2]) == "Clipboard (trimmed)")
    #expect(tags[3].verbatim && Template.chipTone(tags[3]) == .error)
    #expect(Template.buildTag("field",attrs:[Attribute(name:"name",value:.string("x")),Attribute(name:"default",value:.string(""))]) == "{{field name=\"x\"}}")
    #expect(Template.plainSummary("a  \n {{cursor}}") == "a ‹Cursor›")
    #expect(Template.splitOptions(#"a\|b||c|"#) == ["a|b","","c",""])
    #expect(Template.joinOptions(["a|b","c"]) == #"a\|b|c"#)
}
@Test func searchUsesExactUTF16ForSubstringsAndUsageRefreshes() {
    let s = Snippet(name:"Other",triggers:["é"],content:"x e\u{301} x")
    let index = SearchIndex(snippets:[s])
    #expect(index.search("e\u{301}").first?.field == .content)
    #expect(index.search("é").first?.field == .trigger)
    var cached = SearchIndex(snippets:[s]); cached.update(snippets:[s],usage:[s.id:SnippetUsage(count:10)])
    #expect(cached.entries[0].usage == 10 && cached.entries[0].contentLower == index.entries[0].contentLower)
    #expect(Template.splitOptions("a\\|\u{301}|b") == ["a|\u{301}","b"])
}
