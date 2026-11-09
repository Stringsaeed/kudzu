// The library window's own state and the actions that need UI around them (toasts, focus,
// dialogs): `requestDelete` and the group-settings dialog of `main-app.tsx`, inline rename and new
// groups from `sidebar.tsx`.

import Foundation
import KudzuCore
import KudzuDesign
import Observation

@MainActor @Observable
final class LibraryActions {
    let model: AppModel
    let focus: LibraryFocus

    /// The group whose settings dialog is open.
    var groupSettings: SnippetGroup?
    /// The group being renamed inline in the sidebar.
    var renaming: UUID?

    init(model: AppModel, focus: LibraryFocus) {
        self.model = model
        self.focus = focus
    }

    // MARK: Snippets

    /// Deletes snippets with a 6 s Undo toast, then returns focus to the list.
    func requestDelete(_ ids: [UUID]) {
        guard !ids.isEmpty else { return }
        Task {
            do {
                let removed = try await model.deleteSnippets(ids)
                guard !removed.isEmpty else { return }
                toast(LibraryCopy.deleted(removed), action: ToastButton("Undo") { self.restore(removed) }, duration: 6)
                focus.rows = []
                focus.focusList()
            } catch { showError(error) }
        }
    }

    /// Deletes the list's selection (⌘⌫).
    func deleteSelection() { requestDelete(focus.selection(primary: model.selectedId)) }

    func newSnippet(_ draft: SnippetDraft = SnippetDraft()) {
        perform { _ = try await self.model.newSnippet(draft) }
    }

    func duplicate(_ id: UUID) { perform { try await self.model.duplicate(id) } }
    func toggleFavorite(_ id: UUID) { perform { try await self.model.toggleFavorite(id) } }
    func toggleEnabled(_ id: UUID) { perform { try await self.model.toggleEnabled(id) } }
    func move(_ ids: [UUID], to group: UUID?) { perform { try await self.model.moveSnippets(ids, to: group) } }

    /// Keeps failed undo recoverable rather than consuming the only copy of the deleted items.
    private func restore(_ snippets: [Snippet]) {
        Task {
            do { try await model.restoreSnippets(snippets) }
            catch {
                toast.error("Couldn't restore snippets", description: AppModel.errorMessage(error),
                            action: ToastButton("Retry") { self.restore(snippets) }, duration: .infinity)
            }
        }
    }

    func perform(_ action: @escaping @MainActor () async throws -> Void) {
        Task {
            do { try await action() }
            catch { showError(error) }
        }
    }

    private func showError(_ error: Error) {
        toast.error("Couldn't update the library", description: AppModel.errorMessage(error))
    }

    // MARK: Groups

    /// Creates "New group" (under `parent`) and starts renaming it, as the sidebar's + does.
    func newGroup(parent: UUID? = nil, rename: Bool = LibraryGroupCreationSource.sidebar.startsRename) {
        perform {
            let group = try await self.model.createGroup(name: "New group", parentId: parent)
            if rename { self.renaming = group.id }
        }
    }

    func commitRename(_ id: UUID, name: String?) {
        if renaming == id { renaming = nil }
        guard let name else { return }
        saveRename(id, name: name)
    }

    private func saveRename(_ id: UUID, name: String) {
        Task {
            do { try await model.renameGroup(id, to: name) }
            catch {
                toast.error("Couldn't rename group", description: AppModel.errorMessage(error),
                            action: ToastButton("Retry") { self.saveRename(id, name: name) }, duration: .infinity)
            }
        }
    }

    func openGroupSettings(_ group: SnippetGroup) { groupSettings = group }

    /// Moves a dragged item onto a group (`nil` = the Groups heading, i.e. top level / no group).
    @discardableResult
    func drop(_ items: [LibraryDrag], on group: UUID?) -> Bool {
        var handled = false
        for item in items {
            switch item.payload {
            case let .snippets(ids):
                move(ids, to: group)
                handled = true
            case let .group(id):
                guard GroupMoves.canMove(id, into: group, groups: model.library.groups) else { continue }
                perform { _ = try await self.model.moveGroup(id, into: group) }
                handled = true
            }
        }
        return handled
    }

    // MARK: Engine

    func togglePaused() { perform { try await self.model.setPaused(!self.model.status.paused) } }
}
