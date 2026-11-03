import Foundation
import KudzuCore
import Testing
@testable import EditorModels

@MainActor
struct LibraryUndoTests {
    @Test func undoAfterDeleteRestoresTheTextTheUserWasEditing() throws {
        let store = try KudzuStore.inMemory()
        let original = SnippetRecord(name: "Undo me", triggers: [";u"], content: "Saved")
        try store.upsert(original)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let manager = UndoManager()
        manager.groupsByEvent = false
        library.undoManager = manager
        let editor = library.editor(for: original)
        editor.edit { $0.content = "Current edited text" }
        library.deleteSnippets([original.id])
        #expect(try store.library().snippets.isEmpty)
        manager.undo()
        #expect(try store.library().snippets.first?.content == "Current edited text")
        #expect(library.editableSnippet(original.id)?.content == "Current edited text")
        editor.stop()
    }

    @Test func editedContentSurvivesFailedDeleteAfterPreparation() throws {
        enum Failure: Error { case unavailable }
        let store = try KudzuStore.inMemory()
        let original = SnippetRecord(name: "Draft", triggers: [";d"], content: "Saved")
        try store.upsert(original)
        let library = LibraryModel(store: store, removeSnippets: { _ in throw Failure.unavailable })
        library.ensureLoaded()
        let editor = library.editor(for: original)
        editor.edit { $0.content = "Pending draft" }
        library.deleteSnippets([original.id])
        #expect(editor.draft.content == "Pending draft")
        #expect(library.snippetsByID[original.id]?.content == "Pending draft")
        #expect(try store.library().snippets.first?.content == "Pending draft")
    }

    @Test func failedDraftSavePreventsDeletion() throws {
        enum Failure: Error { case unavailable }
        let store = try KudzuStore.inMemory()
        let original = SnippetRecord(name: "Draft", triggers: [";d"], content: "Saved")
        try store.upsert(original)
        var deletionAttempted = false
        let library = LibraryModel(store: store, writeSnippet: { _ in throw Failure.unavailable },
                                   removeSnippets: { ids in
            deletionAttempted = true
            return try store.deleteSnippets(ids: ids)
        })
        library.ensureLoaded()
        let editor = library.editor(for: original)
        editor.edit { $0.content = "Retain me" }
        library.deleteSnippets([original.id])
        #expect(!deletionAttempted)
        #expect(editor.isDirty)
        #expect(editor.draft.content == "Retain me")
        #expect(try store.library().snippets == [original])
    }
    @Test func failedUndoDoesNotAdvanceToRedo() throws {
        let store = try KudzuStore.inMemory()
        var snippet = SnippetRecord(name: "S", triggers: [";s"], content: "Original")
        try store.upsert(snippet)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let manager = UndoManager()
        manager.groupsByEvent = false
        library.undoManager = manager
        manager.beginUndoGrouping()
        library.deleteSnippets([snippet.id])
        manager.endUndoGrouping()
        snippet.content = "Reappeared remotely"
        try store.upsert(snippet)
        manager.undo()
        #expect(!manager.canRedo)
        #expect(library.errorMessage != nil)
        #expect(try store.library().snippets == [snippet])
    }

    @Test func redoPreservesPendingDraftAndRefusesSavedChanges() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "S", triggers: [";s"], content: "Original")
        try store.upsert(snippet)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let manager = UndoManager()
        manager.groupsByEvent = false
        library.undoManager = manager
        manager.beginUndoGrouping()
        library.deleteSnippets([snippet.id])
        manager.endUndoGrouping()
        manager.undo()
        let editor = library.editor(for: snippet)
        editor.edit { $0.content = "Pending draft" }
        manager.redo()
        #expect(editor.draft.content == "Pending draft")
        #expect(library.editableSnippet(snippet.id)?.content == "Pending draft")
        #expect(try store.library().snippets.first?.content == "Original")
        #expect(library.errorMessage != nil)
    }

    @Test func moveRedoPreservesSurvivingEditorAndItsSavedContent() throws {
        let store = try KudzuStore.inMemory()
        let group = GroupRecord(name: "G")
        let snippet = SnippetRecord(name: "S", triggers: [";s"], content: "Original", groupID: group.id)
        try store.upsert(group)
        try store.upsert(snippet)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let manager = UndoManager()
        manager.groupsByEvent = false
        library.undoManager = manager
        manager.beginUndoGrouping()
        library.deleteGroup(group.id, .moveContentsUp)
        manager.endUndoGrouping()
        manager.undo()
        let editor = library.editor(for: snippet)
        editor.edit { $0.content = "Surviving draft" }
        manager.redo()
        #expect(editor.isDirty)
        #expect(editor.draft.content == "Surviving draft")
        #expect(library.editor(for: snippet) === editor)
        #expect(try store.library().snippets.first?.groupID == group.id)
        #expect(editor.flush())
        #expect(try store.library().snippets.first?.content == "Surviving draft")
    }

    @Test func retryFailedUndoIsVisibleAndBecomesANewUndoableRestore() throws {
        let store = try KudzuStore.inMemory()
        var snippet = SnippetRecord(name: "S", triggers: [";s"], content: "Original")
        try store.upsert(snippet)
        try store.recordUsage(snippetID: snippet.id, charsSaved: 9)
        let usage = try store.library().usage
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let manager = UndoManager()
        manager.groupsByEvent = false
        library.undoManager = manager
        manager.beginUndoGrouping()
        library.deleteSnippets([snippet.id])
        manager.endUndoGrouping()
        snippet.content = "Reappeared remotely"
        try store.upsert(snippet)
        manager.undo()
        let retry = try #require(library.deletionRetries.first)
        #expect(library.deletionRetries.count == 1)
        #expect(!manager.canRedo)
        #expect(retry.title == "Retry Restore")
        library.errorMessage = nil
        #expect(library.deletionRetries.count == 1)
        library.retryLibraryDeletion(retry.id)
        #expect(library.deletionRetries.count == 1)
        #expect(try store.library().snippets == [snippet])
        try store.deleteSnippet(id: snippet.id)
        let retryAgain = try #require(library.deletionRetries.first)
        library.retryLibraryDeletion(retryAgain.id)
        #expect(library.deletionRetries.isEmpty)
        #expect(library.errorMessage == nil)
        #expect(try store.library().snippets.first?.content == "Original")
        #expect(try store.library().usage == usage)
        #expect(manager.canUndo)
        #expect(manager.undoActionName == "Restore Deleted Items")
        manager.undo()
        #expect(try store.library().snippets.isEmpty)
        manager.redo()
        #expect(try store.library().snippets.first?.content == "Original")
        #expect(try store.library().usage == usage)
    }

    @Test func failedRedoCanRetryAfterExplicitDraftDiscard() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "S", triggers: [], content: "Original")
        try store.upsert(snippet)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let manager = UndoManager()
        manager.groupsByEvent = false
        library.undoManager = manager
        manager.beginUndoGrouping()
        library.deleteSnippets([snippet.id])
        manager.endUndoGrouping()
        manager.undo()
        let editor = library.editor(for: snippet)
        editor.edit { $0.content = "Later draft" }
        manager.redo()
        let retry = try #require(library.deletionRetries.first)
        #expect(retry.title == "Retry Delete")
        #expect(editor.draft.content == "Later draft")
        #expect(editor.discardChanges())
        library.retryLibraryDeletion(retry.id)
        #expect(library.deletionRetries.isEmpty)
        #expect(try store.library().snippets.isEmpty)
        manager.undo()
        #expect(try store.library().snippets == [snippet])
    }

    @Test func savedDraftKeepsRedoRetryBlockedWithoutLosingTheNewContent() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "S", triggers: [], content: "Original")
        try store.upsert(snippet)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let manager = UndoManager()
        manager.groupsByEvent = false
        library.undoManager = manager
        manager.beginUndoGrouping()
        library.deleteSnippets([snippet.id])
        manager.endUndoGrouping()
        manager.undo()
        let editor = library.editor(for: snippet)
        editor.edit { $0.content = "Saved change" }
        #expect(editor.flush())
        manager.redo()
        let retry = try #require(library.deletionRetries.first)
        library.retryLibraryDeletion(retry.id)
        #expect(library.deletionRetries.count == 1)
        #expect(try store.library().snippets.first?.content == "Saved change")
        #expect(library.editor(for: snippet) === editor)
    }

    @Test func nativeGroupMoveUndoRedoRetainsSurvivorEditor() throws {
        let store = try KudzuStore.inMemory()
        let group = GroupRecord(name: "G")
        let snippet = SnippetRecord(name: "S", triggers: [], content: "Original", groupID: group.id)
        try store.upsert(group)
        try store.upsert(snippet)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let manager = UndoManager()
        manager.groupsByEvent = false
        library.undoManager = manager
        let editor = library.editor(for: snippet)
        manager.beginUndoGrouping()
        library.deleteGroup(group.id, .moveContentsUp)
        manager.endUndoGrouping()
        #expect(editor.draft.groupID == nil)
        manager.undo()
        #expect(editor.draft.groupID == group.id)
        manager.redo()
        #expect(editor.draft.groupID == nil)
        #expect(library.editor(for: snippet) === editor)
        editor.edit { $0.content = "Edited after redo" }
        #expect(editor.flush())
        #expect(try store.library().snippets.first?.content == "Edited after redo")
    }

}
