import Foundation
import KudzuCore
import Testing
import SwiftData
@testable import EditorModels

@MainActor
struct DraftPersistenceTests {
    private func fixture() throws -> (KudzuStore, LibraryModel, SnippetRecord, SnippetEditorModel) {
        let store = try KudzuStore.inMemory()
        let original = SnippetRecord(name: "Audit", triggers: [";audit"], content: "Saved text")
        try store.upsert(original)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        return (store, library, original, SnippetEditorModel(snippet: original, library: library))
    }

    @Test func duplicateUsesCurrentDraft() throws {
        let (_, library, original, editor) = try fixture()
        editor.edit { $0.content = "Current text on screen" }
        let copies = library.duplicate([original.id])
        let id = try #require(copies.first)
        #expect(library.snippetsByID[id]?.content == "Current text on screen")
        editor.stop()
    }

    @Test func favoriteCommandDoesNotLoseDirtyContentOrFavorite() throws {
        let (store, library, original, editor) = try fixture()
        editor.edit { $0.content = "Current text on screen" }
        library.update([original.id]) { $0.isFavorite = true }
        editor.adoptExternal(try #require(library.snippetsByID[original.id]))
        editor.flush()
        let saved = try #require(store.library().snippets.first)
        #expect(saved.isFavorite)
        #expect(saved.content == "Current text on screen")
        editor.stop()
    }

    @Test func externalDeletionDoesNotResurrectOnStop() throws {
        let (store, library, original, editor) = try fixture()
        editor.edit { $0.content = "Unsaved text" }
        try store.deleteSnippet(id: original.id)
        library.reload()
        editor.stop()
        #expect(try store.library().snippets.isEmpty)
        #expect(editor.isDirty)
    }

    @Test func restoredIDCanBeEdited() throws {
        let (store, library, original, editor) = try fixture()
        library.deleteSnippets([original.id])
        try store.upsert(original)
        library.reload()
        editor.adoptExternal(original)
        editor.edit { $0.content = "Edit after restore" }
        editor.flush()
        #expect(try store.library().snippets.first?.content == "Edit after restore")
        #expect(!editor.isDirty)
        editor.stop()
    }
}

@MainActor
struct DraftFailureTests {
    enum Failure: Error { case unavailable }

    private func record() -> SnippetRecord {
        SnippetRecord(name: "Original", triggers: [";original"], content: "Saved")
    }

    @Test func quitSaveIncludesInactiveFailedDraftsAndCanBeRetried() throws {
        let store = try KudzuStore.inMemory()
        let first = record()
        let second = SnippetRecord(name: "Second", triggers: [";second"], content: "Before")
        try store.upsert(first)
        try store.upsert(second)
        var failFirst = true
        let library = LibraryModel(store: store, writeSnippet: { record in
            if failFirst && record.id == first.id { throw Failure.unavailable }
            try store.upsert(record)
        })
        library.ensureLoaded()
        let failed = library.editor(for: first)
        failed.edit { $0.content = "Retained after navigation" }
        failed.stop()
        let active = library.editor(for: second)
        active.edit { $0.content = "Active draft" }

        #expect(!library.savePendingDrafts())
        #expect(failed.isDirty)
        #expect(try store.library().snippets.first { $0.id == second.id }?.content == "Active draft")
        failFirst = false
        #expect(library.savePendingDrafts())
        #expect(try store.library().snippets.first { $0.id == first.id }?.content == "Retained after navigation")
        #expect(library.recoveryDrafts.isEmpty)
    }

    @Test func failedSaveSurvivesNavigationAndStopsCommands() throws {
        let store = try KudzuStore.inMemory()
        let original = record()
        try store.upsert(original)
        var fail = true
        let library = LibraryModel(store: store, writeSnippet: {
            if fail { throw Failure.unavailable }
            try store.upsert($0)
        })
        library.ensureLoaded()
        let editor = library.editor(for: original)
        editor.edit { $0.content = "Keep this draft" }
        editor.stop()
        let reopened = library.editor(for: original)
        #expect(reopened === editor)
        #expect(reopened.draft.content == "Keep this draft")
        #expect(reopened.isDirty)
        #expect(library.recoveryDrafts.map(\.id) == [original.id])
        #expect(library.duplicate([original.id]).isEmpty)
        library.update([original.id]) { $0.isFavorite = true }
        #expect(try store.library().snippets.first == original)
        fail = false
        #expect(reopened.flush())
        #expect(try store.library().snippets.first?.content == "Keep this draft")
        #expect(library.recoveryDrafts.isEmpty)
    }

    @Test func externalConflictIsRetainedAndDiscardNeverOverwritesLatest() throws {
        let store = try KudzuStore.inMemory()
        let original = record()
        try store.upsert(original)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let editor = library.editor(for: original)
        editor.edit { $0.content = "Local" }
        var remote = original
        remote.content = "Remote"
        remote.isFavorite = true
        try store.upsert(remote)
        library.reload()
        #expect(!editor.flush())
        editor.stop()
        #expect(editor.draft.content == "Local")
        #expect(library.editor(for: remote) === editor)
        // An additional external change after the error must also survive Discard.
        remote.name = "Newest remote name"
        try store.upsert(remote)
        #expect(editor.discardChanges())
        #expect(editor.draft == remote)
        #expect(try store.library().snippets.first == remote)
        #expect(!editor.isDirty)
    }

    @Test func discardReadFailureRetainsDraft() throws {
        let store = try KudzuStore.inMemory()
        let original = record()
        try store.upsert(original)
        let library = LibraryModel(store: store, readSnippet: { _ in throw Failure.unavailable })
        let editor = library.editor(for: original)
        editor.edit { $0.content = "Keep me" }
        #expect(!editor.discardChanges())
        #expect(editor.isDirty)
        #expect(editor.draft.content == "Keep me")
        #expect(try store.library().snippets.first == original)
    }

    @Test func failedDeleteKeepsThePreparedEdit() throws {
        let store = try KudzuStore.inMemory()
        let original = record()
        try store.upsert(original)
        let library = LibraryModel(store: store, removeSnippets: { _ in
            for model in try store.context.fetch(FetchDescriptor<SnippetModel>()) {
                store.context.delete(model)
            }
            throw Failure.unavailable
        })
        library.ensureLoaded()
        let editor = library.editor(for: original)
        editor.edit { $0.content = "Survive failed delete" }
        library.deleteSnippets([original.id])
        #expect(editor.draft.content == "Survive failed delete")
        #expect(library.snippetsByID[original.id]?.content == "Survive failed delete")
        #expect(editor.flush())
        #expect(try store.library().snippets.first?.content == "Survive failed delete")
    }

    @Test func groupDeleteFailureKeepsDraftAndMovePreservesDraft() throws {
        let store = try KudzuStore.inMemory()
        let group = GroupRecord(name: "Group")
        try store.upsert(group)
        var original = record()
        original.groupID = group.id
        try store.upsert(original)
        let library = LibraryModel(
            store: store,
            removeSnippets: { _ in throw Failure.unavailable },
            deleteGroupContents: { _ in throw Failure.unavailable }
        )
        library.ensureLoaded()
        let editor = library.editor(for: original)
        editor.edit { $0.content = "Still editing" }
        library.deleteGroup(group.id, .deleteContents)
        #expect(editor.draft.content == "Still editing")
        #expect(try store.library().snippets.first?.content == "Still editing")
        #expect(library.tree.group(group.id) != nil)
        library.deleteGroup(group.id, .moveContentsUp)
        #expect(editor.draft.groupID == nil)
        #expect(editor.flush())
        let saved = try #require(store.library().snippets.first)
        #expect(saved.content == "Still editing")
        #expect(saved.groupID == nil)
    }

    @Test func externalDeleteWithoutReloadCannotResurrectAndDraftRemainsReachable() throws {
        let store = try KudzuStore.inMemory()
        let original = record()
        try store.upsert(original)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let editor = library.editor(for: original)
        editor.edit { $0.content = "Recover me" }
        try store.deleteSnippet(id: original.id)
        #expect(!editor.flush())
        library.reload()
        #expect(library.editableSnippet(original.id)?.content == "Recover me")
        let browser = BrowserState()
        browser.selection = [original.id]
        browser.prune(using: library)
        #expect(browser.selection == [original.id])
        #expect(editor.discardChanges())
        browser.prune(using: library)
        #expect(browser.selection.isEmpty)
        #expect(try store.library().snippets.isEmpty)
    }

    @Test func pendingDraftSurvivesMissingRecordUntilRestore() throws {
        let store = try KudzuStore.inMemory()
        let original = record()
        try store.upsert(original)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let editor = library.editor(for: original)
        editor.edit { $0.content = "Pending" }
        try store.deleteSnippet(id: original.id)
        library.reload()
        #expect(!editor.flush())
        // Repeated reloads while the record is absent must not acknowledge the draft.
        library.reload()
        #expect(!editor.flush())
        #expect(editor.isDirty)
        try store.upsert(original)
        library.reload()
        #expect(editor.flush())
        #expect(try store.library().snippets.first?.content == "Pending")
    }

    @Test func successfulLocalDeleteDoesNotResurrectAndFreshRestoreEditsSave() throws {
        let store = try KudzuStore.inMemory()
        let original = record()
        try store.upsert(original)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let editor = library.editor(for: original)
        editor.edit { $0.content = "Discard with confirmed deletion" }
        library.deleteSnippets([original.id])
        editor.stop()
        #expect(try store.library().snippets.isEmpty)
        #expect(library.recoveryDrafts.isEmpty)
        try store.upsert(original)
        library.reload()
        let restored = library.editor(for: original)
        restored.edit { $0.content = "Restored and edited" }
        #expect(restored.flush())
        #expect(try store.library().snippets.first?.content == "Restored and edited")
    }
}

@MainActor
struct DraftSaveBoundaryTests {
    @Test func editsStayDiscardableUntilSave() async throws {
        let store = try KudzuStore.inMemory()
        let original = SnippetRecord(name: "Name", triggers: [";a"], content: "Saved")
        try store.upsert(original)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let editor = library.editor(for: original)
        editor.edit { $0.content = "Unsaved" }
        try await Task.sleep(for: .milliseconds(500))
        #expect(editor.isDirty)
        #expect(try store.library().snippets.first == original)
        #expect(editor.discardChanges())
        #expect(editor.draft == original)
        editor.stop()
        #expect(try store.library().snippets.first == original)
    }

    @Test func saveFailureRollsBackPendingContextChanges() throws {
        let store = try KudzuStore.inMemory()
        let original = SnippetRecord(name: "Name", triggers: [";a"], content: "Saved")
        try store.upsert(original)
        let library = LibraryModel(store: store, writeSnippet: { draft in
            let row = try #require(store.context.fetch(FetchDescriptor<SnippetModel>()).first)
            row.content = draft.content
            throw DraftFailureTests.Failure.unavailable
        })
        library.ensureLoaded()
        let editor = library.editor(for: original)
        editor.edit { $0.content = "Unsaved" }
        #expect(!editor.flush())
        #expect(editor.isDirty)
        #expect(try store.library().snippets.first == original)
        try store.upsert(GroupRecord(name: "Unrelated save"))
        #expect(try store.library().snippets.first == original)
        #expect(editor.draft.content == "Unsaved")
    }

    @Test func deletingGroupHonorsPendingMoveOutOfGroup() throws {
        let store = try KudzuStore.inMemory()
        let oldGroup = GroupRecord(name: "Delete this group")
        let destination = GroupRecord(name: "Keep this group")
        try store.upsert(oldGroup)
        try store.upsert(destination)
        let original = SnippetRecord(name: "Keep", triggers: [";keep"], content: "Saved", groupID: oldGroup.id)
        try store.upsert(original)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let editor = library.editor(for: original)
        editor.edit {
            $0.groupID = destination.id
            $0.content = "Moved and edited"
        }
        library.deleteGroup(oldGroup.id, .deleteContents)
        let saved = try #require(store.library().snippets.first)
        #expect(saved.groupID == destination.id)
        #expect(saved.content == "Moved and edited")
        #expect(library.tree.group(oldGroup.id) == nil)
    }

}
