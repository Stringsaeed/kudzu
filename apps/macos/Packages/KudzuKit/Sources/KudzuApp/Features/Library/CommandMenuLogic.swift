// The command menu's catalogue and filter (`command-menu.tsx`), the sidebar's remembered
// section collapse (`kudzu-sidebar-sections`), and which clipboard filters the sidebar shows.
// Kept pure so the view can hand the visible rows to `CommandList`.

import Foundation
import KudzuCore

struct CommandAction: Equatable, Identifiable, Sendable {
    enum Section: String, Equatable, Sendable {
        case selected = "Selected snippet"
        case actions = "Actions"
        case goTo = "Go to"
    }

    var id: String
    var label: String
    var keywords: String
    var shortcut: String?
    var section: Section
}

enum CommandMenuLogic {
    /// The web uses the raw query for the hit limit: 5 when empty, otherwise 8, even whitespace.
    static func snippetLimit(_ query: String) -> Int {
        query.isEmpty ? 5 : 8
    }

    /// The heading above those hits.
    static func snippetHeading(_ query: String) -> String {
        query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "Frequently used" : "Snippets"
    }

    /// Every token of `query` must occur in the label or keywords. An empty query hides group
    /// navigation (`g-`) and group settings (`gs-`), which only appear once the user is searching.
    static func visible(_ actions: [CommandAction], query: String) -> [CommandAction] {
        let q = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if q.isEmpty {
            return actions.filter { action in
                if action.section == .goTo, action.id.hasPrefix("g-") { return false }
                if action.id.hasPrefix("gs-") { return false }
                return true
            }
        }
        let terms = q.split { $0.isWhitespace }.map(String.init)
        return actions.filter { action in
            let hay = "\(action.label) \(action.keywords)".lowercased()
            return terms.allSatisfy { hay.contains($0) }
        }
    }

    /// The menu's actions for the current selection, engine and groups. Order matches the web
    /// (selected snippet, then actions, then go-to, with one entry per group at the end).
    static func actions(selected: Snippet?, paused: Bool, theme: Theme, clipboardEnabled: Bool, groups: [SnippetGroup]) -> [CommandAction] {
        var list: [CommandAction] = []
        if let selected {
            let name = selected.name.isEmpty ? "Untitled" : selected.name
            list.append(CommandAction(id: "fav", label: selected.isFavorite ? "Unfavorite “\(name)”" : "Favorite “\(name)”", keywords: "", shortcut: "mod+shift+f", section: .selected))
            list.append(CommandAction(id: "en", label: selected.isEnabled ? "Turn off “\(name)”" : "Turn on “\(name)”", keywords: "", shortcut: "mod+shift+e", section: .selected))
            list.append(CommandAction(id: "dup", label: "Duplicate “\(name)”", keywords: "", shortcut: "mod+d", section: .selected))
            list.append(CommandAction(id: "del", label: "Delete “\(name)”", keywords: "", shortcut: "mod+backspace", section: .selected))
        }
        list.append(CommandAction(id: "new", label: "New snippet", keywords: "create add", shortcut: "mod+n", section: .actions))
        list.append(CommandAction(id: "newg", label: "New group", keywords: "folder create", shortcut: "mod+shift+n", section: .actions))
        list.append(CommandAction(id: "pause", label: paused ? "Resume expanding" : "Pause expanding", keywords: "stop disable", shortcut: "mod+shift+p", section: .actions))
        list.append(CommandAction(id: "imp", label: "Import snippets…", keywords: "raycast textexpander espanso json yaml csv", shortcut: nil, section: .actions))
        list.append(CommandAction(id: "exp", label: "Export library…", keywords: "backup json espanso", shortcut: nil, section: .actions))
        list.append(CommandAction(id: "theme", label: theme == .dark ? "Switch to light appearance" : "Switch to dark appearance", keywords: "theme mode color", shortcut: nil, section: .actions))
        list.append(CommandAction(id: "keys", label: "Keyboard shortcuts", keywords: "help keys", shortcut: "mod+/", section: .actions))
        list.append(CommandAction(id: "all", label: "All snippets", keywords: "", shortcut: "mod+1", section: .goTo))
        list.append(CommandAction(id: "favs", label: "Favorites", keywords: "", shortcut: "mod+2", section: .goTo))
        list.append(CommandAction(id: "recent", label: "Recently used", keywords: "", shortcut: "mod+3", section: .goTo))
        list.append(CommandAction(id: "ins", label: "Insights", keywords: "stats statistics usage", shortcut: "mod+4", section: .goTo))
        list.append(CommandAction(id: "clip", label: "Clipboard history", keywords: "copy paste history copied", shortcut: "mod+5", section: .goTo))
        list.append(CommandAction(id: "clipset", label: "Settings › Clipboard", keywords: "history retention keep days privacy", shortcut: nil, section: .goTo))
        list.append(CommandAction(id: "clippause", label: clipboardEnabled ? "Pause clipboard history" : "Resume clipboard history", keywords: "copy record privacy stop", shortcut: nil, section: .actions))
        list.append(CommandAction(id: "set", label: "Settings", keywords: "preferences", shortcut: "mod+,", section: .goTo))
        list.append(CommandAction(id: "apps", label: "Settings › Apps", keywords: "exclude block terminal", shortcut: nil, section: .goTo))
        list.append(CommandAction(id: "data", label: "Settings › Backups & data", keywords: "restore import export", shortcut: nil, section: .goTo))
        for node in Groups.flatten(Groups.buildTree(groups)) {
            let path = Groups.groupPath(groups, id: node.group.id).joined(separator: " › ")
            list.append(CommandAction(id: "g-\(node.group.id.uuidString)", label: path, keywords: "group folder", shortcut: nil, section: .goTo))
            list.append(CommandAction(id: "gs-\(node.group.id.uuidString)", label: "Group settings: \(path)", keywords: "rules apps", shortcut: nil, section: .actions))
        }
        return list
    }
}

/// Which feature sections are collapsed, remembered on this Mac (`localStorage` key
/// `kudzu-sidebar-sections`: a JSON array of section ids).
enum SidebarSections {
    static let storageKey = "kudzu-sidebar-sections"

    static func decode(_ raw: String?) -> Set<String> {
        guard let raw, let data = raw.data(using: .utf8),
              let ids = try? JSONDecoder().decode([String].self, from: data) else { return [] }
        return Set(ids)
    }

    static func encode(_ closed: Set<String>) -> String {
        let data = (try? JSONEncoder().encode(closed.sorted())) ?? Data("[]".utf8)
        return String(decoding: data, as: UTF8.self)
    }
}

/// Clipboard rows under the sidebar's Clipboard heading.
enum ClipSidebar {
    /// Pinned is always listed. Other kinds show when they have items, or when that filter is open.
    static func visibleFilters(counts: [String: Int], active: String?) -> [Clipboard.Filter] {
        Clipboard.FILTERS.filter { filter in
            filter.value != "all" && (filter.value == "pinned" || (counts[filter.value] ?? 0) > 0 || active == filter.value)
        }
    }

    static func counts(_ entries: [ClipEntry]) -> [String: Int] {
        Dictionary(uniqueKeysWithValues: Clipboard.FILTERS.map { filter in
            let n = filter.value == "all" ? entries.count : entries.filter { Clipboard.inFilter($0, filter: filter.value) }.count
            return (filter.value, n)
        })
    }
}
