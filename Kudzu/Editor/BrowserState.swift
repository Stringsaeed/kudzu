import Foundation
import KudzuCore
import KudzuUI
import Observation
import SwiftUI
import UniformTypeIdentifiers

/// What the sidebar selects.
enum SidebarScope: Hashable, RawRepresentable {
    case all
    case favorites
    case recent
    case stats
    case group(UUID)

    init?(rawValue: String) {
        switch rawValue {
        case "all": self = .all
        case "favorites": self = .favorites
        case "recent": self = .recent
        case "stats": self = .stats
        default:
            guard rawValue.hasPrefix("group:"), let id = UUID(uuidString: String(rawValue.dropFirst(6))) else { return nil }
            self = .group(id)
        }
    }

    var rawValue: String {
        switch self {
        case .all: "all"
        case .favorites: "favorites"
        case .recent: "recent"
        case .stats: "stats"
        case .group(let id): "group:\(id.uuidString)"
        }
    }

    var groupID: UUID? {
        if case .group(let id) = self { id } else { nil }
    }
}

/// An in-app drag. SwiftUI drop delegates can't read item providers synchronously, so the
/// source records what it is dragging here and the providers only carry a marker type.
enum DragPayload: Equatable {
    case snippets([UUID])
    case group(UUID)
}

extension UTType {
    /// Marker type attached to in-app sidebar/list drags.
    static let kudzuDrag = UTType(exportedAs: "sh.saeed.kudzu.drag", conformingTo: .data)
}

extension NSItemProvider {
    /// A provider carrying only the marker type; the real payload lives in `BrowserState.dragging`.
    static func kudzuDragMarker() -> NSItemProvider {
        let provider = NSItemProvider()
        provider.registerDataRepresentation(forTypeIdentifier: UTType.kudzuDrag.identifier, visibility: .ownProcess) { completion in
            completion(Data(), nil)
            return nil
        }
        return provider
    }
}

/// Where keyboard and menu focus should move. The pane that owns it consumes the request.
enum LibraryPaneRequest: Equatable {
    case sidebar
    case search
    case results
    case editor(EditorFocusTarget)
}

/// An uncommitted trigger entry kept on the window so it survives switching snippets.
struct TriggerEntry: Equatable {
    var text: String
    var error: String?
}

/// Commands the open editor exposes so menus can discard or edit the draft instead of the saved snippet.
struct EditorCommandBridge {
    var owner: UUID
    var save: () -> Void
    var copyExpanded: () -> Void
    var copySource: () -> Void
    var discard: () -> Void
    var toggleFavorite: () -> Void
    var delete: () -> Void
}

/// Per-window browsing state shared by the sidebar, list and editor.
@MainActor @Observable
final class BrowserState {
    var scope: SidebarScope = .all
    var selection: Set<UUID> = []
    var query = ""
    var dragging: DragPayload?
    /// Snippets waiting for the user to confirm their deletion; the window shows the dialog while it's non-empty.
    var deleteRequest: [UUID] = []
    /// Bumped when a selection change is refused, so the list redraws the selection it already has.
    private(set) var selectionEpoch = 0
    /// A selection the user asked for while the editor refused to leave. Applied by `completeDeferredNavigation()`.
    private(set) var deferredSelection: Set<UUID>?
    /// Uncommitted trigger text and its rejection, keyed by snippet.
    var uncommittedTriggers: [UUID: TriggerEntry] = [:]
    /// Focus request for the sidebar, search field, results, or editor. The matching view clears it.
    var paneRequest: LibraryPaneRequest?
    /// Incremented by Find so the search field selects its text.
    var searchSelectAllToken = 0
    /// True while the open editor has unsaved edits. Menus read this; Escape does not.
    var editorCanDiscard = false
    /// Installed by the editor on screen. Return false to keep the current snippet selected.
    ///
    /// The editor's installer returns `!isDirty` and does not save. A save or discard UI can replace
    /// the guard, return false while it asks, then call `completeDeferredNavigation()` once the user
    /// has saved or discarded. Escape never calls this and never discards edits.
    @ObservationIgnored var editorLeaveGuard: (() -> Bool)?
    @ObservationIgnored private var leaveGuardOwner: UUID?
    @ObservationIgnored var editorBridge: EditorCommandBridge?

    /// Asks to delete snippets; the window confirms first. IDs that no longer exist are dropped.
    func requestDelete(_ ids: [UUID], in library: LibraryModel) {
        let existing = ids.filter { library.snippetsByID[$0] != nil }
        guard !existing.isEmpty else { return }
        deleteRequest = existing
    }

    /// The snippets a command acts on: the selection when `id` is part of it, otherwise just `id`.
    func targets(for id: UUID) -> [UUID] {
        selection.contains(id) ? Array(selection) : [id]
    }

    /// Selects a snippet, switching away from a scope or search that would hide it.
    func reveal(_ id: UUID, in library: LibraryModel) {
        guard let snippet = library.snippetsByID[id] else { return }
        guard editorLeaveGuard?() ?? true else { return }
        query = ""
        switch scope {
        case .all: break
        case .group(let groupID):
            let inScope = snippet.groupID.map { library.tree.descendantIDs(of: groupID).contains($0) } ?? false
            if !inScope { scope = .all }
        case .favorites:
            if !snippet.isFavorite { scope = .all }
        case .recent, .stats:
            scope = .all
        }
        replaceSelection([id])
    }

    /// Creates a snippet and selects it. It goes in `groupID` if given, otherwise in the selected group (if any).
    ///
    /// The editor focuses the name field (or the trigger field when `focus` says so) once it appears.
    @discardableResult
    func newSnippet(
        library: LibraryModel,
        in groupID: UUID? = nil,
        content: String = "",
        nameHint: String? = nil,
        focus: EditorFocusTarget = .name
    ) -> Bool {
        guard editorLeaveGuard?() ?? true else { return false }
        let target = groupID ?? scope.groupID
        guard let id = library.createSnippet(in: target, content: content, nameHint: nameHint) else { return false }
        library.pendingFocus = focus
        query = ""
        if let target {
            scope = .group(target)
        } else if scope != .all {
            scope = .all
        }
        replaceSelection([id])
        return true
    }

    /// Keeps the active editor visible when a failed save prevents a sidebar change.
    func chooseScope(_ next: SidebarScope) {
        guard next != scope, editorLeaveGuard?() ?? true else { return }
        scope = next
    }

    /// Moves the selection, or remembers it when the editor refuses to leave.
    ///
    /// The same selection applies immediately and cancels a deferred move: the user stayed.
    /// A refused move keeps `selection` unchanged so the dirty editor is not torn down.
    func replaceSelection(_ proposed: Set<UUID>) {
        if proposed == selection {
            deferredSelection = nil
            return
        }
        let allowsLeave = editorLeaveGuard?() ?? true
        switch KzNavigationGate.decide(current: selection, proposed: proposed, allowsLeave: allowsLeave) {
        case .apply(let next):
            deferredSelection = nil
            selection = next
        case .hold(let next):
            // A repeat of the same refused selection must not bump the epoch, or the list refresh
            // writes it again and the window never settles.
            let changed = deferredSelection != next
            deferredSelection = next
            if changed { selectionEpoch += 1 }
        }
    }

    /// Applies `deferredSelection` without asking the leave guard again.
    func completeDeferredNavigation() {
        guard let next = deferredSelection else { return }
        deferredSelection = nil
        if next != selection { selection = next }
    }

    /// Installs the leave guard for the editor of `owner`. A later editor's install wins over an earlier one's removal.
    func installLeaveGuard(owner: UUID, _ allowsLeave: @escaping () -> Bool) {
        leaveGuardOwner = owner
        editorLeaveGuard = allowsLeave
    }

    /// Removes the guard only when `owner` is still the editor that installed it.
    func removeLeaveGuard(owner: UUID) {
        guard leaveGuardOwner == owner else { return }
        leaveGuardOwner = nil
        editorLeaveGuard = nil
    }

    func triggerText(for id: UUID) -> Binding<String> {
        Binding(
            get: { self.uncommittedTriggers[id]?.text ?? "" },
            set: { self.updateTriggerEntry(id: id, text: $0, error: self.uncommittedTriggers[id]?.error) }
        )
    }

    func triggerError(for id: UUID) -> Binding<String?> {
        Binding(
            get: { self.uncommittedTriggers[id]?.error },
            set: { self.updateTriggerEntry(id: id, text: self.uncommittedTriggers[id]?.text ?? "", error: $0) }
        )
    }

    func clearTriggerEntry(_ id: UUID) {
        uncommittedTriggers[id] = nil
    }

    /// Commits a valid pending trigger before leaving, saving, exporting or quitting.
    @discardableResult
    func commitTriggerEntry(_ id: UUID, in library: LibraryModel) -> Bool {
        guard let entry = uncommittedTriggers[id] else { return true }
        let candidate = entry.text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !candidate.isEmpty else { clearTriggerEntry(id); return true }
        guard let snippet = library.editableSnippet(id) else {
            library.errorMessage = "The snippet for an unfinished trigger no longer exists. Review or clear that trigger before continuing."
            return false
        }
        if let error = LibraryModel.triggerRejection(for: candidate, existing: snippet.triggers, caseSensitive: snippet.caseMode == .exact) {
            uncommittedTriggers[id] = TriggerEntry(text: entry.text, error: error)
            library.errorMessage = "Fix or clear the unfinished trigger in “\(snippet.name)” before continuing."
            return false
        }
        library.editor(for: snippet).edit { $0.triggers.append(candidate) }
        clearTriggerEntry(id)
        return true
    }

    /// Attempts all entries so each invalid trigger keeps its own explanation.
    func commitPendingTriggers(in library: LibraryModel) -> Bool {
        Array(uncommittedTriggers.keys).map { commitTriggerEntry($0, in: library) }.allSatisfy { $0 }
    }

    /// Drops selection and scope entries that no longer exist after the library changed.
    ///
    /// This does not ask the leave guard: the snippets are already gone.
    func prune(using library: LibraryModel) {
        let live = Set(library.snippetsByID.keys).union(library.recoveryDrafts.map(\.id))
        selection.formIntersection(live)
        if var deferred = deferredSelection {
            deferred.formIntersection(live)
            deferredSelection = deferred
        }
        if case .group(let id) = scope, library.tree.group(id) == nil { scope = .all }
    }

    private func updateTriggerEntry(id: UUID, text: String, error: String?) {
        let blank = text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        if blank && error == nil {
            uncommittedTriggers[id] = nil
        } else {
            uncommittedTriggers[id] = TriggerEntry(text: text, error: error)
        }
    }
}
