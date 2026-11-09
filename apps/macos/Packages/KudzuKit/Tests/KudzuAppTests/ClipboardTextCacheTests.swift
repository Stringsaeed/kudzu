import Foundation
import KudzuCore
import Testing
@testable import KudzuApp

@MainActor @Suite struct ClipboardTextCacheTests {
    @Test func fetchedTextBelongsToItsModelAndIsEvictedOnDeletion() async throws {
        let firstBackend = MockBackend(options: .init())
        let secondBackend = MockBackend(options: .init())
        let firstModel = AppModel(backend: firstBackend)
        let secondModel = AppModel(backend: secondBackend)
        let first = ClipEntry(kind: .text, text: "First store's complete text", textLen: 27, hash: "same-id-in-two-stores", firstCopiedAt: .distantPast, copiedAt: .distantPast)
        var second = first
        second.text = "Second store's complete text"
        await firstBackend.clipboardRestore([first])
        await secondBackend.clipboardRestore([second])
        var excerpt = first
        excerpt.text = "excerpt"
        #expect(await firstModel.clipFullText(excerpt) == "First store's complete text")
        #expect(await secondModel.clipFullText(excerpt) == "Second store's complete text")

        _ = await firstBackend.clipboardDelete(ids: [first.id])
        firstModel.applyClipEvent(.remove(ids: [first.id]))
        await firstBackend.clipboardRestore([second])
        #expect(await firstModel.clipFullText(excerpt) == "Second store's complete text")
    }

    @Test func unavailableFullTextCanBeRetriedWhenTheBlobReturns() async {
        let backend = MockBackend()
        let model = AppModel(backend: backend)
        let entry = ClipEntry(kind: .text, text: "Complete text recovered later", textLen: 29, hash: "recoverable-blob", firstCopiedAt: .distantPast, copiedAt: .distantPast)
        var excerpt = entry
        excerpt.text = "Complete"
        #expect(await model.clipFullText(excerpt) == "Complete")
        await backend.clipboardRestore([entry])
        #expect(await model.clipFullText(excerpt) == "Complete text recovered later")
    }
}
