import Foundation
import KudzuCore

struct LibraryCommandRow: Equatable, Identifiable {
    var id: String
    var title: String
    var heading: String
    var shortcut: String?
    var groupPath: String?
    var trigger: String?
    var snippetID: UUID?
}

/// The web disables cmdk filtering: snippet search and action-token filtering are authoritative.
enum LibraryCommandRows {
    static func rows(query: String, library: Library, usage: [UUID: SnippetUsage], actions: [CommandAction]) -> [LibraryCommandRow] {
        let hits = SearchIndex(snippets: library.snippets, usage: usage).search(query, limit: CommandMenuLogic.snippetLimit(query))
        let snippets = hits.map { hit in
            let s = hit.snippet
            return LibraryCommandRow(
                id: "snippet-\(s.id.uuidString)",
                title: s.name.isEmpty ? "Untitled" : s.name,
                heading: CommandMenuLogic.snippetHeading(query),
                groupPath: Groups.groupPath(library.groups, id: s.groupID).joined(separator: " › "),
                trigger: s.triggers.first,
                snippetID: s.id
            )
        }
        let visible = CommandMenuLogic.visible(actions, query: query)
        let commands = [CommandAction.Section.selected, .actions, .goTo].flatMap { section in
            visible.filter { $0.section == section }.map { action in
                LibraryCommandRow(id: action.id, title: action.label, heading: section.rawValue, shortcut: action.shortcut)
            }
        }
        return snippets + commands
    }

    /// cmdk's loop navigation wraps at both ends.
    static func moved(_ selected: String?, by delta: Int, rows: [LibraryCommandRow]) -> String? {
        guard !rows.isEmpty else { return nil }
        let index = rows.firstIndex { $0.id == selected } ?? (delta > 0 ? -1 : 0)
        let next = ((index + delta) % rows.count + rows.count) % rows.count
        return rows[next].id
    }
}
