import Foundation
import KudzuCore
import Testing
@testable import EditorModels

@MainActor
struct BrowserNavigationTests {
    enum Failure: Error { case unavailable }

    @Test func duplicateIncludesAValidUnfinishedTrigger() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "First", triggers: [";one"], content: "Text")
        try store.upsert(snippet)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let browser = BrowserState()
        browser.triggerText(for: snippet.id).wrappedValue = ";alias"
        let ids = browser.duplicateSnippets([snippet.id], in: library)
        let id = try #require(ids.first)
        #expect(library.snippetsByID[id]?.triggers == [";one2", ";alias2"])
        #expect(library.snippetsByID[snippet.id]?.triggers == [";one", ";alias"])
        #expect(browser.uncommittedTriggers.isEmpty)
    }

    @Test func duplicateWithInvalidOrComposingInputCreatesNothing() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "First", triggers: [";one"], content: "Text")
        try store.upsert(snippet)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let browser = BrowserState()
        browser.triggerText(for: snippet.id).wrappedValue = "invalid trigger"
        #expect(browser.duplicateSnippets([snippet.id], in: library).isEmpty)
        #expect(try store.library().snippets == [snippet])
        #expect(browser.uncommittedTriggers[snippet.id]?.text == "invalid trigger")
        browser.triggerText(for: snippet.id).wrappedValue = "かな"
        browser.inputIsComposing = { true }
        #expect(!browser.commitTriggerEntry(snippet.id, in: library))
        #expect(!browser.commitPendingTriggers(in: library))
        #expect(browser.duplicateSnippets([snippet.id], in: library).isEmpty)
        #expect(try store.library().snippets == [snippet])
        #expect(browser.uncommittedTriggers[snippet.id]?.text == "かな")
    }

    @Test func leavingCommitsCurrentTextAndPendingTriggerBeforeChangingSelection() throws {
        let store = try KudzuStore.inMemory()
        let first = SnippetRecord(name: "First", triggers: [";one"], content: "Before")
        let second = SnippetRecord(name: "Second", triggers: [";two"], content: "Other")
        try store.upsert(snippets: [first, second])
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let browser = BrowserState()
        browser.selection = [first.id]
        browser.triggerText(for: first.id).wrappedValue = ";alias"
        let editor = library.editor(for: first)
        editor.edit { $0.content = "Current text" }
        browser.installLeaveGuard(owner: first.id) {
            browser.commitTriggerEntry(first.id, in: library) && editor.flush()
        }
        browser.replaceSelection([second.id])
        let saved = try #require(store.library().snippets.first { $0.id == first.id })
        #expect(saved.content == "Current text")
        #expect(saved.triggers == [";one", ";alias"])
        #expect(browser.selection == [second.id])
        #expect(browser.uncommittedTriggers.isEmpty)
        browser.removeLeaveGuard(owner: first.id)
    }

    @Test func invalidTriggerStaysRecoverableAcrossSaveBoundaries() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "First", triggers: [";one"], content: "Body")
        try store.upsert(snippet)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let browser = BrowserState()
        browser.triggerText(for: snippet.id).wrappedValue = "invalid trigger"
        #expect(!browser.commitPendingTriggers(in: library))
        #expect(browser.uncommittedTriggers[snippet.id]?.text == "invalid trigger")
        #expect(browser.uncommittedTriggers[snippet.id]?.error != nil)
        #expect(try store.library().snippets == [snippet])
        browser.clearTriggerEntry(snippet.id)
        #expect(browser.commitPendingTriggers(in: library))
    }

    @Test func failedNavigationCanClearSelectionAfterExplicitDiscard() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "First", triggers: [";one"], content: "Before")
        try store.upsert(snippet)
        let library = LibraryModel(store: store, writeSnippet: { _ in throw Failure.unavailable })
        library.ensureLoaded()
        let browser = BrowserState()
        let editor = library.editor(for: snippet)
        editor.edit { $0.content = "Unsaved" }
        browser.selection = [snippet.id]
        browser.installLeaveGuard(owner: snippet.id) { editor.flush() }
        browser.replaceSelection([])
        let epoch = browser.selectionEpoch
        browser.replaceSelection([])
        #expect(browser.selectionEpoch == epoch)
        #expect(browser.selection == [snippet.id])
        browser.prune(using: library)
        #expect(browser.deferredSelection == [])
        #expect(editor.discardChanges())
        browser.completeDeferredNavigation()
        #expect(browser.selection.isEmpty)
        #expect(try store.library().snippets == [snippet])
        browser.removeLeaveGuard(owner: snippet.id)
    }

    @Test func refusedNavigationDoesNotCreateAHiddenSnippetOrSwitchScope() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "First", triggers: [";one"], content: "Saved")
        try store.upsert(snippet)
        let library = LibraryModel(store: store)
        library.ensureLoaded()
        let browser = BrowserState()
        browser.selection = [snippet.id]
        browser.installLeaveGuard(owner: snippet.id) { false }
        #expect(!browser.newSnippet(library: library))
        browser.chooseScope(.stats)
        #expect(browser.scope == .all)
        #expect(browser.selection == [snippet.id])
        #expect(try store.library().snippets == [snippet])
        browser.removeLeaveGuard(owner: snippet.id)
    }
}
