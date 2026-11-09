// Drag and drop inside the library (`SNIPPET_DRAG_TYPE` / `GROUP_DRAG_TYPE` in `sidebar.tsx`):
// snippets onto groups, groups onto groups (nesting) or the Groups heading (top level). Dragged out
// of Kudzu, snippets become their text.

import CoreTransferable
import Foundation
import KudzuCore
import UniformTypeIdentifiers

extension UTType {
    /// `application/x-kudzu-snippets` / `application/x-kudzu-group`, as one type.
    /// T12: declare it under `UTExportedTypeDeclarations` in the app's Info.plist (conforms to
    /// `public.data`) so the system stops warning that it is undeclared.
    static let kudzuLibraryItem = UTType(exportedAs: "sh.saeed.kudzu.library-item", conformingTo: .data)
}

/// What a library drag carries.
struct LibraryDrag: Codable, Transferable, Hashable, Sendable {
    enum Payload: Codable, Hashable, Sendable {
        case snippets([UUID])
        case group(UUID)
    }

    var payload: Payload
    /// Plain text for other apps: the snippets' content, or the group's name.
    var text: String

    static var transferRepresentation: some TransferRepresentation {
        CodableRepresentation(contentType: .kudzuLibraryItem)
        ProxyRepresentation(exporting: \.text)
    }

    static func snippets(_ ids: [UUID], in library: Library) -> LibraryDrag {
        let chosen = Set(ids)
        let text = library.snippets.filter { chosen.contains($0.id) }.map(\.content).joined(separator: "\n\n")
        return LibraryDrag(payload: .snippets(ids), text: text)
    }

    static func group(_ group: SnippetGroup) -> LibraryDrag {
        LibraryDrag(payload: .group(group.id), text: group.name)
    }
}
