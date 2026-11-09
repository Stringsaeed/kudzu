import Foundation
import KudzuCore

/// What the library screens ask of the model beyond `lib/store.ts`'s actions.
extension AppModel {
    /// The snippet shown in the editor.
    public var selectedSnippet: Snippet? { selectedId.flatMap(snippet) }

    /// Whether the main window shows the snippet list and editor (`inLibrary`).
    public var inLibrary: Bool { LibraryRoutes.isLibrary(view) }

    /// Re-parents or reorders a group (`nil` = top level), refusing moves into itself or a descendant
    /// (the `descendantIds` check of `sidebar.tsx`). Routes through `backend.moveGroup` so sibling
    /// `sortIndex` values are reindexed. Authoritative library events publish the result; an
    /// older completion must not replay its move over a newer event.
    @discardableResult
    public func moveGroup(_ id: UUID, into parent: UUID?, index: Int? = nil) async throws -> Bool {
        if index == nil {
            guard GroupMoves.canMove(id, into: parent, groups: library.groups) else { return false }
        } else {
            guard library.groups.contains(where: { $0.id == id }),
                  !LibraryEditing.wouldCycle(library.groups, id: id, parentID: parent) else { return false }
        }
        let targetIndex = index ?? library.groups.filter { $0.parentID == parent && $0.id != id }.count
        try await backend.moveGroup(id: id, parentId: parent, index: targetIndex)
        return true
    }

    /// Renames a group; blank or unchanged names are ignored (`RenameRow`'s `onDone`).
    public func renameGroup(_ id: UUID, to name: String) async throws {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, var group = library.groups.first(where: { $0.id == id }), group.name != trimmed else { return }
        group.name = trimmed
        try await saveGroup(group)
    }

    /// "Turn off group" / "Turn on group".
    public func toggleGroupEnabled(_ id: UUID) async throws {
        guard var group = library.groups.first(where: { $0.id == id }) else { return }
        group.isEnabled.toggle()
        try await saveGroup(group)
    }

    /// The bulk bar's "Add to favorites": favourites every selected snippet that isn't yet.
    public func favoriteAll(_ ids: [UUID]) async throws {
        for id in ids {
            guard var s = snippet(id), !s.isFavorite else { continue }
            s.isFavorite = true
            try await saveSnippet(s)
        }
    }

    /// The bulk bar's "Toggle on/off": flips each selected snippet, as the web did.
    public func toggleEnabledEach(_ ids: [UUID]) async throws {
        for id in ids { try await toggleEnabled(id) }
    }
}
