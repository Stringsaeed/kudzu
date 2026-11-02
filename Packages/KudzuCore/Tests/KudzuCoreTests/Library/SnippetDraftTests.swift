import Testing
@testable import KudzuCore

struct SnippetDraftTests {
    @Test func mergesIndependentFields() throws {
        let base = SnippetRecord(name: "Name", triggers: [";a"], content: "Saved")
        var draft = base
        draft.content = "Local"
        var latest = base
        latest.isFavorite = true
        let merged = try SnippetDraft.merge(draft, baseline: base, latest: latest)
        #expect(merged.content == "Local")
        #expect(merged.isFavorite)
    }

    @Test func refusesOverlappingEditsAndDeletion() {
        let base = SnippetRecord(name: "Name", triggers: [";a"], content: "Saved")
        var draft = base
        draft.content = "Local"
        var latest = base
        latest.content = "Remote"
        #expect(throws: SnippetDraft.Conflict.self) { try SnippetDraft.merge(draft, baseline: base, latest: latest) }
        #expect(throws: SnippetDraft.Conflict.self) { try SnippetDraft.merge(draft, baseline: base, latest: nil) }
    }

    @Test func identicalConcurrentEditsAreNotAConflict() throws {
        let base = SnippetRecord(name: "Name", triggers: [";a"], content: "Saved")
        var draft = base
        draft.content = "Same change"
        #expect(try SnippetDraft.merge(draft, baseline: base, latest: draft) == draft)
    }
}
