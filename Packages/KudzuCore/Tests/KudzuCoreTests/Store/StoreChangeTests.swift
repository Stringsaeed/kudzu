import Foundation
import Testing
@testable import KudzuCore

@MainActor
@Suite(.timeLimit(.minutes(1))) struct StoreChangeTests {
    @Test func localSavesAreCoalescedIntoOneChange() async throws {
        let store = try KudzuStore.inMemory()
        let changes = store.changes
        let start = store.changeToken
        for i in 0..<5 {
            try store.upsert(SnippetRecord(name: "\(i)", triggers: [";\(i)"], content: "\(i)"))
        }
        var iterator = changes.makeAsyncIterator()
        #expect(await iterator.next() != nil)
        #expect(store.changeToken == start + 1)

        // Nothing else pending: a short wait sees no further change.
        try await Task.sleep(for: .milliseconds(300))
        #expect(store.changeToken == start + 1)
    }

    @Test func remoteChangeRunsDedupeAndHook() async throws {
        let store = try KudzuStore.inMemory()
        var hookCalls = 0
        store.onRemoteChange = { hookCalls += 1 }
        let a = SnippetRecord(name: "a", triggers: [";x"], content: "X", createdAt: Date(timeIntervalSinceReferenceDate: 1))
        let b = SnippetRecord(name: "b", triggers: [";x"], content: "X", createdAt: Date(timeIntervalSinceReferenceDate: 2))
        try store.replaceAll(with: KudzuLibrary(snippets: [a, b]))
        let changes = store.changes
        var iterator = changes.makeAsyncIterator()
        // replaceAll's own save notification.
        #expect(await iterator.next() != nil)

        store.noteRemoteChange()
        #expect(await iterator.next() != nil)
        #expect(hookCalls == 1)
        #expect(try store.library().snippets.map(\.id) == [a.id])
    }

    @Test func multipleSubscribersEachGetNotified() async throws {
        let store = try KudzuStore.inMemory()
        let first = store.changes, second = store.changes
        try store.upsert(SnippetRecord(name: "s", triggers: [";s"], content: "S"))
        var i1 = first.makeAsyncIterator(), i2 = second.makeAsyncIterator()
        #expect(await i1.next() != nil)
        #expect(await i2.next() != nil)
    }
}
