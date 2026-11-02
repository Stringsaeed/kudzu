import Foundation
import KudzuCore
import Observation
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

/// Per-window browsing state shared by the sidebar, list and editor.
@MainActor @Observable
final class BrowserState {
    var scope: SidebarScope = .all
    var selection: Set<UUID> = []
    var query = ""
    var dragging: DragPayload?
    /// Snippets waiting for the user to confirm their deletion; the window shows the dialog while it's non-empty.
    var deleteRequest: [UUID] = []

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
        selection = [id]
    }

    /// Creates a snippet and selects it. It goes in `groupID` if given, otherwise in the selected group (if any).
    ///
    /// The editor focuses the name field (or the trigger field when `focus` says so) once it appears.
    func newSnippet(
        library: LibraryModel,
        in groupID: UUID? = nil,
        content: String = "",
        nameHint: String? = nil,
        focus: EditorFocusTarget = .name
    ) {
        let target = groupID ?? scope.groupID
        guard let id = library.createSnippet(in: target, content: content, nameHint: nameHint) else { return }
        library.pendingFocus = focus
        query = ""
        if let target {
            scope = .group(target)
        } else if scope != .all {
            scope = .all
        }
        selection = [id]
    }

    /// Drops selection and scope entries that no longer exist after the library changed.
    func prune(using library: LibraryModel) {
        selection.formIntersection(library.snippetsByID.keys)
        if case .group(let id) = scope, library.tree.group(id) == nil { scope = .all }
    }
}
