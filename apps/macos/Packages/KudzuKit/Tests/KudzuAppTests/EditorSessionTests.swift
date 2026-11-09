import Foundation
import KudzuCore
import Testing
@testable import KudzuApp

@Suite("Editor draft boundaries") @MainActor
struct EditorSessionTests {
    @Test func autosaveDefersUntilCompositionCommitsNaturally() async throws {
        let model = AppModel(backend: MockBackend())
        await model.start()
        defer { model.stop() }
        let first = try #require(model.library.snippets.first)
        var commits = 0
        var writes = 0
        var composing = true
        let session = EditorSession(model: model, persist: { _ in writes += 1 })
        await session.select(first.id)
        session.beforeFlush = { commits += 1 }
        session.isComposing = { composing }
        session.update { $0.content = "Ordinary edit before IME" }
        try await Task.sleep(for: .milliseconds(500))
        #expect(commits == 0 && writes == 0)
        #expect(session.isDirty)
        composing = false
        session.update { $0.content = "Naturally committed 日本" }
        #expect(await session.flush())
        #expect(commits == 1 && writes == 1)
    }

    @Test func returningToCurrentDraftCancelsPendingNavigation() async throws {
        let model = AppModel(backend: MockBackend(), isDev: true)
        await model.start()
        defer { model.stop() }
        let first = try #require(model.library.snippets.first)
        let other = try #require(model.library.snippets.last)
        var resumeSave: CheckedContinuation<Void, Never>?
        let session = EditorSession(model: model, persist: { snippet in
            await withCheckedContinuation { resumeSave = $0 }
            try await model.saveSnippet(snippet)
        })
        await session.select(first.id)
        session.update { $0.content = "Changed" }
        let navigation = Task { await session.select(other.id) }
        while resumeSave == nil { await Task.yield() }
        await session.select(first.id)
        resumeSave?.resume()
        await navigation.value
        #expect(model.selectedId == first.id)
        #expect(session.draft?.id == first.id)
    }
    @Test func savesThroughModelBeforeChangingSelection() async throws {
        let model = AppModel(backend: MockBackend(), isDev: true)
        await model.start()
        defer { model.stop() }
        let first = try #require(model.library.snippets.first)
        let other = try #require(model.library.snippets.last)
        let session = model.editorSession
        await session.select(first.id)
        session.update { $0.content = "Latest text" }
        await session.select(first.id)
        #expect(session.draft?.content == "Latest text")
        #expect(session.isDirty)
        await session.select(other.id)
        #expect(model.selectedId == other.id)
        let stored = try await model.backend.getState()
        #expect(stored.library.snippets.first { $0.id == first.id }?.content == "Latest text")
        #expect(!session.isDirty)
    }

    @Test func deletedDraftIsNeverSentByDisappearFlush() async throws {
        let model = AppModel(backend: MockBackend(), isDev: true)
        await model.start()
        defer { model.stop() }
        let first = try #require(model.library.snippets.first)
        var writes = 0
        let session = EditorSession(model: model, persist: { _ in writes += 1 })
        await session.select(first.id)
        session.update { $0.content = "Unsaved text" }
        _ = try await model.deleteSnippets([first.id])
        #expect(await session.flush())
        #expect(writes == 0)
        #expect(session.draft == nil)
        #expect(model.snippet(first.id) == nil)
    }

    @Test func failedSavePinsSelectionAndDiscardRestoresSavedText() async throws {
        enum Failure: Error { case disk }
        let model = AppModel(backend: MockBackend(), isDev: true)
        await model.start()
        defer { model.stop() }
        let first = try #require(model.library.snippets.first)
        let other = try #require(model.library.snippets.last)
        let session = EditorSession(model: model, persist: { snippet in
            model.upsertLocal(snippet) // The model optimistically updates before a failing backend.
            throw Failure.disk
        })
        await session.select(first.id)
        session.update { $0.content = "Recoverable text" }
        await session.select(other.id)
        #expect(model.selectedId == first.id)
        #expect(session.draft?.content == "Recoverable text")
        #expect(session.error != nil)
        #expect(session.isDirty)
        session.discard()
        #expect(session.draft?.content == first.content)
        #expect(model.snippet(first.id)?.content == first.content)
    }

    @Test func editsDuringSaveAreSerializedAndPersistLatestRevision() async throws {
        let model = AppModel(backend: MockBackend(), isDev: true)
        await model.start()
        defer { model.stop() }
        let first = try #require(model.library.snippets.first)
        var writes: [String] = []
        var session: EditorSession?
        let editor = EditorSession(model: model, persist: { snippet in
            writes.append(snippet.content)
            if writes.count == 1 { session?.update { $0.content = "Second revision" } }
            try await model.saveSnippet(snippet)
        })
        session = editor
        await editor.select(first.id)
        editor.update { $0.content = "First revision" }
        #expect(await editor.flush())
        #expect(writes == ["First revision", "Second revision"])
        let stored = try await model.backend.getState()
        #expect(stored.library.snippets.first { $0.id == first.id }?.content == "Second revision")
        session = nil
    }

    @Test func flushCommitsTheHookBeforeWritingTheDraft() async throws {
        let model = AppModel(backend: MockBackend(), isDev: true)
        await model.start()
        defer { model.stop() }
        let first = try #require(model.library.snippets.first)
        let session = model.editorSession
        await session.select(first.id)
        session.update { $0.content = "Draft" }
        session.beforeFlush = { session.update { $0.content = "Composed 日本" } }
        #expect(await session.flush())
        let stored = try await model.backend.getState()
        #expect(stored.library.snippets.first { $0.id == first.id }?.content == "Composed 日本")
        #expect(!session.isDirty)
        session.beforeFlush = nil
    }

    @Test func leavingASnippetRunsTheHookSoMarkedTextIsSaved() async throws {
        let model = AppModel(backend: MockBackend(), isDev: true)
        await model.start()
        defer { model.stop() }
        let first = try #require(model.library.snippets.first)
        let other = try #require(model.library.snippets.last)
        let session = model.editorSession
        await session.select(first.id)
        session.beforeFlush = { session.update { $0.content = "Composed" } }
        await session.select(other.id)
        let stored = try await model.backend.getState()
        #expect(stored.library.snippets.first { $0.id == first.id }?.content == "Composed")
        #expect(session.draft?.id == other.id)
        session.beforeFlush = nil
    }
}
