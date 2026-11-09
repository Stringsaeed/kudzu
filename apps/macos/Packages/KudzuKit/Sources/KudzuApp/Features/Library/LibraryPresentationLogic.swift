import Foundation
import KudzuCore

/// Presentation rules shared by library rows and their keyboard navigation.
enum LibraryRowSummary {
    static func text(_ snippet: Snippet, library: Library) -> String {
        let names = Dictionary(library.snippets.map { ($0.id, $0.name) }, uniquingKeysWith: { _, name in name })
        return Template.plainSummary(snippet.content) { UUID(uuidString: $0).flatMap { names[$0] } }
    }
}

struct SnippetListNavigation: Equatable {
    enum Focus: Hashable { case filter, list }
    var selection: ListSelection
    var focus: Focus

    func moved(in ids: [UUID], by delta: Int, extend: Bool) -> Self {
        Self(selection: selection.moved(in: ids, by: delta, extend: extend), focus: focus)
    }
}

/// Sidebar rename gestures, independent of native event delivery.
enum GroupRenameGesture {
    static func startsRename(clickCount: Int = 0, key: String = "", option: Bool = false) -> Bool {
        clickCount == 2 || key == "\u{F705}" || (key == "\r" && option)
    }
}

/// Window shortcuts must leave native text-editing key equivalents to the field/editor.
enum LibraryKeyboardPolicy {
    static func canDeleteSelection(inLibrary: Bool, editingText: Bool) -> Bool { inLibrary && !editingText }

    static func opensShortcuts(character: String, command: Bool, control: Bool, option: Bool, editingText: Bool) -> Bool {
        character == "?" && !command && !control && !option && !editingText
    }
}

/// Sidebar creation immediately renames; window/menu commands simply create the group.
enum LibraryGroupCreationSource {
    case windowShortcut, commandMenu, sidebar
    var startsRename: Bool { self == .sidebar }
}
