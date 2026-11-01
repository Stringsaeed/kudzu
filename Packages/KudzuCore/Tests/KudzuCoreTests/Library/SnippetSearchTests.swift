import Foundation
import Testing
@testable import KudzuCore

@Suite("SnippetSearchIndex")
struct SnippetSearchTests {
    private func s(_ name: String, _ triggers: [String] = [], _ content: String = "", updated: TimeInterval = 0) -> SnippetRecord {
        SnippetRecord(name: name, triggers: triggers, content: content, updatedAt: Date(timeIntervalSince1970: updated))
    }

    @Test func emptyQueryReturnsEverythingInOrder() {
        let all = [s("b"), s("a")]
        #expect(SnippetSearchIndex(snippets: all).filter(all, query: "  ") == all)
    }

    @Test func matchesNameTriggerAndContent() {
        let a = s("Signature", [";sig"], "Best regards"), b = s("Address", [";addr"], "1 Main St")
        let index = SnippetSearchIndex(snippets: [a, b])
        #expect(index.filter([a, b], query: "signa") == [a])
        #expect(index.filter([a, b], query: ";addr") == [b])
        #expect(index.filter([a, b], query: "main") == [b])
        #expect(index.filter([a, b], query: "zzz").isEmpty)
    }

    @Test func caseAndDiacriticInsensitive() {
        let a = s("Café Menu", [";café"], "Crème brûlée")
        let index = SnippetSearchIndex(snippets: [a])
        #expect(index.filter([a], query: "CAFE") == [a])
        #expect(index.filter([a], query: "creme brulee") == [a])
        #expect(index.filter([a], query: "CRÈME") == [a])
    }

    @Test func allTermsMustMatchInAnyOrderAndField() {
        let a = s("Signature", [";sig"], "Best regards")
        let index = SnippetSearchIndex(snippets: [a])
        #expect(index.filter([a], query: "regards sig") == [a])
        #expect(index.filter([a], query: "regards nope").isEmpty)
    }

    @Test func updateAndRemoveReflectEdits() {
        var a = s("Old", [";o"])
        var index = SnippetSearchIndex(snippets: [a])
        #expect(index.filter([a], query: "new").isEmpty)
        a.name = "New"
        index.update(a)
        #expect(index.filter([a], query: "new") == [a])
        index.remove(a.id)
        #expect(index.filter([a], query: "new").isEmpty)
    }

    @Test func sortsByNameIgnoringCaseAndDiacritics() {
        let all = [s("banana"), s("Äpfel"), s("Cherry"), s("apple")]
        let sorted = SnippetSearchIndex(snippets: all).sorted(all, by: .name, stats: [:])
        #expect(sorted.map(\.name) == ["Äpfel", "apple", "banana", "Cherry"])
    }

    @Test func sortsByRecentlyEdited() {
        let old = s("old", updated: 1), new = s("new", updated: 9)
        let index = SnippetSearchIndex(snippets: [old, new])
        #expect(index.sorted([old, new], by: .recentlyEdited, stats: [:]).map(\.name) == ["new", "old"])
    }

    @Test func sortsByMostUsedWithNameTieBreak() {
        let a = s("a"), b = s("b"), c = s("c")
        let stats = [a.id: SnippetStats(snippetID: a.id, count: 1), b.id: SnippetStats(snippetID: b.id, count: 5)]
        let index = SnippetSearchIndex(snippets: [a, b, c])
        #expect(index.sorted([c, a, b], by: .mostUsed, stats: stats).map(\.name) == ["b", "a", "c"])
    }

    @Test func sortsByRecentlyUsedWithNeverUsedLast() {
        let a = s("a"), b = s("b"), c = s("c")
        let stats = [
            a.id: SnippetStats(snippetID: a.id, count: 1, lastUsedAt: Date(timeIntervalSince1970: 100)),
            b.id: SnippetStats(snippetID: b.id, count: 1, lastUsedAt: Date(timeIntervalSince1970: 200)),
        ]
        let index = SnippetSearchIndex(snippets: [a, b, c])
        #expect(index.sorted([c, a, b], by: .recentlyUsed, stats: stats).map(\.name) == ["b", "a", "c"])
    }

    @Test func handlesAFewThousandSnippetsQuickly() {
        let all = (0..<5000).map { s("Snippet \($0)", [";s\($0)"], "Body of snippet number \($0)") }
        let index = SnippetSearchIndex(snippets: all)
        let start = Date()
        let hits = index.filter(all, query: "number 4999")
        let sorted = index.sorted(hits, by: .name, stats: [:])
        #expect(sorted.count == 1)
        #expect(Date().timeIntervalSince(start) < 1.0)
    }
}
