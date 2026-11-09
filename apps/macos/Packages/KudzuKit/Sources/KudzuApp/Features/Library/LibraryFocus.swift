// Keyboard focus and multi-selection across the library panes: the native counterpart of
// `SnippetListHandle` (`focusFilter`, `focusList`, `selection`) and the refs `main-app.tsx` kept to
// the clipboard view and the editor. Views own their `@FocusState`; they follow `request`.

import Foundation
import Observation

/// The library window's focus requests and the list's multi-selection. Shared through the
/// environment (`@Environment(LibraryFocus.self)`), so the editor (T8) and clipboard view (T10) can
/// react to ⌘E / ⌘F / ⌘L and return focus to the list on Esc.
@MainActor @Observable
public final class LibraryFocus {
    /// A pane that can take keyboard focus.
    public enum Target: Hashable, Sendable {
        case filter, list, sidebar
        /// The snippet editor's content (⌘E, Return/→ in the list).
        case editor
        /// The clipboard view's search field and list (⌘F / ⌘L on the Clipboard route).
        case clipboardSearch, clipboardList
    }

    /// The latest request; `serial` changes every time so asking twice for the same pane refocuses.
    public struct Request: Equatable, Sendable {
        public var target: Target
        public var serial: Int
    }

    public private(set) var request: Request?

    /// The list's selected rows (native multi-selection). One row unless extended.
    public var rows: Set<UUID> = []

    public init() {}

    public func focus(_ target: Target) {
        request = Request(target: target, serial: (request?.serial ?? 0) + 1)
    }

    public func focusList() { focus(.list) }
    public func focusFilter() { focus(.filter) }
    public func focusEditor() { focus(.editor) }

    /// Ids currently selected, multi-select aware (`selection()`): every selected row when more
    /// than one is, else the editor's snippet.
    public func selection(primary: UUID?) -> [UUID] {
        if rows.count > 1 { return Array(rows) }
        return primary.map { [$0] } ?? []
    }
}
