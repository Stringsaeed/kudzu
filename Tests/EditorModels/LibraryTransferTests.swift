import Foundation
import KudzuCore
import Testing
@testable import EditorModels

@MainActor
struct LibraryTransferTests {
    enum Failure: Error { case unavailable }

    @Test func transferIncludesTheMountedEditorsCurrentDraft() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "Current", triggers: [";current"], content: "Saved")
        try store.upsert(snippet)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let editor = library.editor(for: snippet)
        editor.edit { $0.content = "Visible edit" }
        let exported = try library.libraryForTransfer()
        #expect(exported.snippets.first?.content == "Visible edit")
        #expect(!editor.isDirty)
    }

    @Test func transferRefusesStaleSnapshotWhenDraftCannotSave() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "Current", triggers: [";current"], content: "Saved")
        try store.upsert(snippet)
        let library = LibraryModel(store: store, writeSnippet: { _ in throw Failure.unavailable })
        library.ensureLoaded()
        let editor = library.editor(for: snippet)
        editor.edit { $0.content = "Keep this edit" }
        #expect(throws: (any Error).self) { try library.libraryForTransfer() }
        #expect(editor.isDirty)
        #expect(editor.draft.content == "Keep this edit")
        #expect(try store.library().snippets == [snippet])
    }

    @Test func failedBatchDuplicateReturnsNoCopyIDsAndRetryCreatesOnlyOneSet() throws {
        let store = try KudzuStore.inMemory()
        let originals = [SnippetRecord(name: "First", triggers: [";first"], content: "A"), SnippetRecord(name: "Second", triggers: [";second"], content: "B")]
        try store.upsert(snippets: originals)
        var fail = true
        let library = LibraryModel(store: store, writeSnippets: { copies in
            if fail {
                store.context.insert(SnippetModel(copies[0]))
                throw Failure.unavailable
            }
            try store.upsert(snippets: copies)
        })
        library.ensureLoaded()
        #expect(library.duplicate(originals.map(\.id)).isEmpty)
        #expect(Set(try store.library().snippets) == Set(originals))
        fail = false
        #expect(library.duplicate(originals.map(\.id)).count == 2)
        #expect(try store.library().snippets.count == 4)
    }
}
