// The library screens' pure logic, kept out of the views so it can be tested: route titles and
// scoping (`snippet-list.tsx`), the sort/filter pipeline, selection movement, bulk-action state,
// group moves with cycle prevention (`sidebar.tsx`), breadcrumbs (`toolbar.tsx`) and engine health
// (`status.tsx`).

import Foundation
import KudzuCore

// MARK: - Routes

enum LibraryRoutes {
    /// Whether the route shows the snippet list and editor (`inLibrary` in `main-app.tsx`).
    static func isLibrary(_ route: Route) -> Bool {
        switch route {
        case .insights, .settings, .clipboard: false
        default: true
        }
    }

    /// The list's heading (`viewTitle`).
    static func title(_ route: Route, groups: [SnippetGroup]) -> String {
        switch route {
        case .all: "All snippets"
        case .favorites: "Favorites"
        case .recent: "Recently used"
        case .disabled: "Turned off"
        case let .group(id): groups.first { $0.id == id }?.name ?? "Group"
        default: ""
        }
    }

    /// The route a sidebar row is tagged with, so `List(selection:)` matches the current route the
    /// way `sameView` did: the clipboard's "all" filter is History, and any settings section
    /// selects the Settings row.
    static func sidebarTag(_ route: Route) -> Route {
        switch route {
        case .clipboard(filter: .all): .clipboard(filter: nil)
        case .settings: .settings(section: nil)
        default: route
        }
    }
}

// MARK: - List pipeline

/// The list's sort menu (`Sort`).
enum SnippetSort: String, CaseIterable, Identifiable, Sendable {
    case relevance, name, edited, used
    var id: String { rawValue }

    var label: String {
        switch self {
        case .relevance: "Best match"
        case .name: "Name"
        case .edited: "Recently edited"
        case .used: "Most used"
        }
    }
}

enum SnippetListLogic {
    /// Snippets in scope for the route (`scoped`).
    static func scoped(_ library: Library, route: Route, usage: [UUID: SnippetUsage]) -> [Snippet] {
        let s = library.snippets
        switch route {
        case .favorites: return s.filter(\.isFavorite)
        case .recent: return s.filter { (usage[$0.id]?.last ?? 0) > 0 }
        case .disabled: return s.filter { !$0.isEnabled }
        case let .group(id):
            let ids = Groups.descendantIds(library.groups, id: id)
            return s.filter { $0.groupID.map(ids.contains) ?? false }
        default: return s
        }
    }

    /// Search, then sort (`hits`): Recently used keeps recency unless a query asks for relevance
    /// otherwise; with no query "Best match" is by name.
    static func hits(_ snippets: [Snippet], usage: [UUID: SnippetUsage], filter: String, sort: SnippetSort, route: Route) -> [SearchHit] {
        let index = SearchIndex(snippets: snippets, usage: usage)
        var r = index.search(filter, limit: 10_000)
        let q = filter.trimmingCharacters(in: .whitespacesAndNewlines)
        func by(_ before: (Snippet, Snippet) -> Bool) { r = stableSorted(r) { before($0.snippet, $1.snippet) } }
        if route == .recent && (q.isEmpty || sort == .relevance) {
            by { (usage[$0.id]?.last ?? 0) > (usage[$1.id]?.last ?? 0) }
        } else if sort == .name || (q.isEmpty && sort == .relevance) {
            // `localeCompare(…, { sensitivity: "base" })`, untitled last.
            by { nameKey($0).compare(nameKey($1), options: [.caseInsensitive, .diacriticInsensitive], locale: .current) == .orderedAscending }
        } else if sort == .edited {
            by { $0.updatedAt > $1.updatedAt }
        } else if sort == .used {
            by { (usage[$0.id]?.count ?? 0) > (usage[$1.id]?.count ?? 0) }
        }
        return r
    }

    private static func nameKey(_ s: Snippet) -> String { s.name.isEmpty ? "\u{FFFF}" : s.name }

    /// `Array.sort` in JavaScript is stable; Swift's `sorted` is not guaranteed to be.
    static func stableSorted<T>(_ items: [T], by before: (T, T) -> Bool) -> [T] {
        items.enumerated().sorted { a, b in
            if before(a.element, b.element) { return true }
            if before(b.element, a.element) { return false }
            return a.offset < b.offset
        }.map(\.element)
    }

    /// The snippet to select when the current selection fell out of the list (the "keep a valid
    /// selection" effect); `nil` leaves the selection alone.
    static func fallbackSelection(ids: [UUID], selectedId: UUID?, filter: String, route: Route, libraryIds: Set<UUID>) -> UUID? {
        guard !ids.isEmpty else { return nil }
        if let selectedId, ids.contains(selectedId) { return nil }
        guard selectedId == nil || filter.isEmpty else { return nil }
        let keep = selectedId.map(libraryIds.contains) == true && filter.isEmpty && route == .all
        return keep ? nil : ids[0]
    }

    /// The draft "Create it" makes from a filter with no matches.
    static func draft(fromFilter filter: String) -> SnippetDraft {
        let q = filter.trimmingCharacters(in: .whitespacesAndNewlines)
        return filter.hasPrefix(";") ? SnippetDraft(triggers: [q]) : SnippetDraft(name: q)
    }

    /// The empty list's copy (`EmptyList`), and whether it offers "New snippet".
    static func emptyCopy(_ route: Route) -> (title: String, body: String, offersNew: Bool) {
        switch route {
        case .favorites: ("No favorites yet", "Star the snippets you use most to find them faster — they also rank first in search.", true)
        case .recent: ("Nothing used yet", "Snippets you expand will show up here.", false)
        case .disabled: ("Everything is on", "Snippets you turn off appear here.", false)
        case .group: ("This group is empty", "Create a snippet here or drag snippets onto the group.", true)
        default: ("Your library is empty", "Snippets turn a short trigger like ;sig into text you type often.", true)
        }
    }
}

// MARK: - Selection

/// Keyboard selection in the list (`move` in `snippet-list.tsx`), used where the native list
/// can't drive it: ↑/↓ in the filter field, J/K, Home/End and Page Up/Down.
struct ListSelection: Equatable, Sendable {
    /// The primary (editor) selection.
    var current: UUID?
    /// Every selected row (one row unless extended).
    var rows: Set<UUID>
    /// Where a shift-extension started.
    var anchor: UUID?

    /// Moves by `delta` rows (clamped); with `extend` the range from the anchor is selected.
    func moved(in ids: [UUID], by delta: Int, extend: Bool) -> ListSelection {
        guard !ids.isEmpty else { return self }
        let index = current.flatMap(ids.firstIndex(of:)) ?? (delta > 0 ? -1 : ids.count)
        let n = max(0, min(ids.count - 1, index + delta))
        let id = ids[n]
        guard extend else { return ListSelection(current: id, rows: [id], anchor: id) }
        let a = anchor ?? current ?? id
        let ai = ids.firstIndex(of: a) ?? n
        let (lo, hi) = ai < n ? (ai, n) : (n, ai)
        return ListSelection(current: id, rows: Set(ids[lo...hi]), anchor: a)
    }

    /// The new primary selection when the list's selection set changes natively: a newly added
    /// row wins (the clicked one), otherwise the old primary if still selected, otherwise the last
    /// selected row in list order.
    static func primary(old: Set<UUID>, new: Set<UUID>, previous: UUID?, ids: [UUID]) -> UUID? {
        if let added = ids.last(where: { new.contains($0) && !old.contains($0) }) { return added }
        if let previous, new.contains(previous) { return previous }
        return ids.last(where: new.contains) ?? new.first
    }
}

// MARK: - Bulk actions

/// The bulk bar's state for a multi-selection (`bulk` in `snippet-list.tsx`).
struct BulkState: Equatable {
    var ids: [UUID]
    /// "Add to favorites" has something to do.
    var canFavorite: Bool
    /// Every selected snippet is on (the toggle reads "Turn off").
    var allEnabled: Bool

    /// `nil` unless more than one snippet is selected (the bar only shows then).
    init?(selection: [UUID], library: Library) {
        guard selection.count > 1 else { return nil }
        let chosen = Set(selection)
        let snippets = library.snippets.filter { chosen.contains($0.id) }
        ids = selection
        canFavorite = snippets.contains { !$0.isFavorite }
        allEnabled = snippets.allSatisfy(\.isEnabled)
    }
}

// MARK: - Group moves

enum GroupMoves {
    /// Whether `moving` can become a child of `target` (`nil` = top level): never into itself or
    /// one of its descendants, and not where it already is.
    static func canMove(_ moving: UUID, into target: UUID?, groups: [SnippetGroup]) -> Bool {
        guard let group = groups.first(where: { $0.id == moving }) else { return false }
        guard let target else { return group.parentID != nil }
        if group.parentID == target { return false }
        return !Groups.descendantIds(groups, id: moving).contains(target)
    }
}

// MARK: - Breadcrumbs

/// One crumb: its label and where it goes (`nil` = the current page).
struct Crumb: Equatable {
    var label: String
    var destination: Route?
}

enum LibraryBreadcrumbs {
    /// The toolbar's trail for the route and selected snippet (`toolbar.tsx`).
    static func crumbs(route: Route, groups: [SnippetGroup], snippet: Snippet?) -> [Crumb] {
        switch route {
        case let .settings(section):
            let s = section ?? .general
            return [Crumb(label: "Settings", destination: .settings(section: nil)), Crumb(label: settingsLabel(s), destination: nil)]
        case .insights:
            return [Crumb(label: "Insights", destination: nil)]
        case let .clipboard(filter):
            let label = filter.flatMap { f in f == .all ? nil : Clipboard.FILTERS.first { $0.value == f.rawValue }?.label } ?? "History"
            return [Crumb(label: "Clipboard", destination: .clipboard(filter: nil)), Crumb(label: label, destination: nil)]
        default:
            var items = [Crumb(label: "Snippets", destination: .all)]
            switch route {
            case .favorites: items.append(Crumb(label: "Favorites", destination: route))
            case .recent: items.append(Crumb(label: "Recently used", destination: route))
            case .disabled: items.append(Crumb(label: "Turned off", destination: route))
            default: break
            }
            var groupId: UUID? = snippet?.groupID
            if groupId == nil, case let .group(id) = route { groupId = id }
            var ancestors: [Crumb] = []
            var seen = Set<UUID>()
            var group = groups.first { $0.id == groupId }
            while let g = group, !seen.contains(g.id) {
                seen.insert(g.id)
                ancestors.insert(Crumb(label: g.name, destination: .group(id: g.id)), at: 0)
                group = groups.first { $0.id == g.parentID }
            }
            items += ancestors
            if let snippet { items.append(Crumb(label: snippet.name.isEmpty ? "Untitled" : snippet.name, destination: nil)) }
            else if route == .all { items.append(Crumb(label: "All snippets", destination: nil)) }
            return items
        }
    }

    /// `SETTINGS_SECTIONS` labels (`settings.tsx`).
    static func settingsLabel(_ section: SettingsSection) -> String {
        switch section {
        case .general: "General"
        case .expansion: "Expansion"
        case .clipboard: "Clipboard"
        case .agents: "AI agents"
        case .apps: "Apps"
        case .data: "Backups & data"
        case .shortcuts: "Shortcuts"
        case .about: "About"
        }
    }
}

// MARK: - Engine health

/// `Health` in `status.tsx`.
enum EngineHealth: Equatable {
    case ready, paused, limited, blocked, secure

    init(status: EngineStatus, platform: Platform) {
        if status.paused { self = .paused }
        else if platform == .web { self = .ready }
        else if status.secureInput { self = .secure }
        else if status.tap == .off || (!status.permissions.inputMonitoring && !status.permissions.accessibility) { self = .blocked }
        else if !status.permissions.accessibility || status.tap == .listenOnly { self = .limited }
        else { self = .ready }
    }

    var label: String {
        switch self {
        case .ready: "Expanding"
        case .paused: "Paused"
        case .limited: "Limited"
        case .blocked: "Needs permission"
        case .secure: "Secure Input on"
        }
    }

    /// The tooltip; names the Secure Input owner when known.
    func detail(owner: String?) -> String {
        switch self {
        case .ready: "Kudzu is listening for your triggers in every app."
        case .paused: "Nothing expands until you resume."
        case .limited: "Grant Accessibility so Kudzu can type expansions reliably."
        case .blocked: "Kudzu can't see your typing yet. Open Settings → Permissions."
        case .secure: owner.flatMap { $0.isEmpty ? nil : $0 }.map { "\($0) has Secure Input on, so macOS blocks expansion." } ?? "A password field or app has Secure Input on, so macOS blocks expansion."
        }
    }

    /// Clicking the status opens Settings for these.
    var opensSettings: Bool { self == .blocked || self == .limited }
    /// The shield shows for these.
    var showsShield: Bool { self == .blocked || self == .secure }
}

// MARK: - Toast copy

enum LibraryCopy {
    /// The deletion toast's title.
    static func deleted(_ removed: [Snippet]) -> String {
        removed.count == 1 ? "Deleted “\(removed[0].name.isEmpty ? "Untitled" : removed[0].name)”" : "Deleted \(removed.count) snippets"
    }

    /// `reportImport`'s title and description.
    static func imported(_ r: ImportSummary) -> (title: String, description: String) {
        let s = r.importedSnippets, g = r.importedGroups
        let what = "\(s) snippet\(s == 1 ? "" : "s")" + (g > 0 ? " in \(g) group\(g == 1 ? "" : "s")" : "")
        let w = r.warnings.count
        let description = w > 0 ? "\(w) item\(w == 1 ? "" : "s") imported with changes — e.g. \(r.warnings[0].message)" : "A backup was saved first, just in case."
        return ("Imported \(what)", description)
    }

    /// The Espanso export toast.
    static func exportedEspanso(_ n: Int, dir: String) -> (title: String, description: String) {
        ("Exported \(n) Espanso file\(n == 1 ? "" : "s")", "\(dir) · Formatting and images export as plain text. Use Kudzu JSON to keep them.")
    }
}
