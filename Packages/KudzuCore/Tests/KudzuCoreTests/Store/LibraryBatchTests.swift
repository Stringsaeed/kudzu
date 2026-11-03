import Testing
@testable import KudzuCore

@MainActor
struct LibraryBatchTests {
    enum Failure: Error { case unavailable }

    @Test func snippetBatchCommitsOnceAndRollsBackEveryRecordOnFailure() throws {
        let store = try KudzuStore.inMemory()
        let original = SnippetRecord(name: "Existing", triggers: [";existing"], content: "Before")
        try store.upsert(original)
        var edited = original
        edited.content = "After"
        let added = SnippetRecord(name: "New", triggers: [";new"], content: "New body")
        var commits = 0
        store.commitChanges = { _ in commits += 1; throw Failure.unavailable }
        #expect(throws: Failure.self) { try store.upsert(snippets: [edited, added]) }
        #expect(commits == 1)
        #expect(try store.library().snippets == [original])
        #expect(!store.context.hasChanges)
        store.commitChanges = { context in commits += 1; try context.save() }
        try store.upsert(snippets: [edited, added])
        #expect(commits == 2)
        #expect(Set(try store.library().snippets) == Set([edited, added]))
    }

    @Test func invalidLaterGroupRollsBackEarlierGroupChange() throws {
        let store = try KudzuStore.inMemory()
        let first = GroupRecord(name: "First")
        let second = GroupRecord(name: "Second")
        try store.upsert(groups: [first, second])
        var moved = first
        moved.name = "Renamed"
        var invalid = second
        invalid.parentID = second.id
        #expect(throws: KudzuStoreError.groupCycle) { try store.upsert(groups: [moved, invalid]) }
        #expect(Set(try store.library().groups) == Set([first, second]))
        #expect(!store.context.hasChanges)
    }
}
