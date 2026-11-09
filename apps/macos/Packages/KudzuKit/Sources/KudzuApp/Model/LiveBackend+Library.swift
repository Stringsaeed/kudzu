import Foundation
import KudzuCore

// Snippet and group commands (`commands.rs` "Snippets" and "Groups").
extension LiveBackend {
    public func createSnippet(draft: SnippetDraft) async throws -> Snippet {
        let now = Date()
        let snippet = try await editLibrary { library in
            LibraryEditing.createSnippet(in: &library, draft: draft, now: now)
        }
        return snippet
    }

    public func saveSnippet(_ snippet: Snippet) async throws -> Snippet {
        let now = Date()
        let saved = try await editLibrary { library in
            try LibraryEditing.saveSnippet(snippet, in: &library, now: now)
        }
        return saved
    }

    public func deleteSnippets(ids: [UUID]) async throws {
        try await editLibrary { library in
            LibraryEditing.deleteSnippets(ids: ids, in: &library)
        }
        store.pruneFormMemoryToCommittedLibrary()
    }

    public func restoreSnippets(_ snippets: [Snippet]) async throws {
        try await editLibrary { library in
            LibraryEditing.restoreSnippets(snippets, in: &library)
        }
    }

    public func duplicateSnippet(id: UUID) async throws -> Snippet {
        let now = Date()
        let copy = try await editLibrary { library in
            try LibraryEditing.duplicateSnippet(id: id, in: &library, now: now)
        }
        return copy
    }

    public func moveSnippets(ids: [UUID], groupId: UUID?) async throws {
        let now = Date()
        try await editLibrary { library in
            try LibraryEditing.moveSnippets(ids: ids, groupID: groupId, in: &library, now: now)
        }
    }

    public func findConflicts() async throws -> [TriggerConflict] {
        let library = store.library
        return KudzuCore.findConflicts(snippets: library.snippets, groups: library.groups)
    }

    // MARK: Groups

    public func createGroup(name: String, parentId: UUID?) async throws -> Group {
        let group = try await editLibrary { library in
            try LibraryEditing.createGroup(name: name, parentID: parentId, in: &library)
        }
        return group
    }

    public func saveGroup(_ group: Group) async throws -> Group {
        let saved = try await editLibrary { library in
            try LibraryEditing.saveGroup(group, in: &library)
        }
        return saved
    }

    public func moveGroup(id: UUID, parentId: UUID?, index: Int) async throws {
        try await editLibrary { library in
            try LibraryEditing.moveGroup(id: id, parentID: parentId, index: index, in: &library)
        }
    }

    public func deleteGroup(id: UUID, deleteSnippets: Bool) async throws {
        let outcome = try await editLibrary { library in
            LibraryEditing.deleteGroup(id: id, deleteSnippets: deleteSnippets, in: &library)
        }
        if !outcome.deletedSnippetIDs.isEmpty {
            store.pruneFormMemoryToCommittedLibrary()
        }
    }
}
