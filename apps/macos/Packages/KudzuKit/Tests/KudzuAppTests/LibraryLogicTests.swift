import Foundation
import KudzuCore
import Testing
@testable import KudzuApp

/// Library behaviour ported from the desktop main window: routes, the list pipeline, selection,
/// bulk actions, group moves, breadcrumbs, engine health, command-menu filtering, and toast copy.
@Suite struct LibraryLogicTests {
    private func snippet(
        _ name: String,
        triggers: [String] = [],
        content: String = "",
        group: UUID? = nil,
        favorite: Bool = false,
        enabled: Bool = true,
        updatedAt: Date = Date(timeIntervalSince1970: 0)
    ) -> Snippet {
        Snippet(
            name: name,
            triggers: triggers,
            content: content,
            groupID: group,
            isFavorite: favorite,
            isEnabled: enabled,
            createdAt: updatedAt,
            updatedAt: updatedAt
        )
    }

    private func names(_ hits: [SearchHit]) -> [String] { hits.map(\.snippet.name) }

    // MARK: Routes

    @Test func libraryRoutesSkipInsightsSettingsAndClipboard() {
        #expect(LibraryRoutes.isLibrary(.all))
        #expect(LibraryRoutes.isLibrary(.favorites))
        #expect(LibraryRoutes.isLibrary(.recent))
        #expect(LibraryRoutes.isLibrary(.disabled))
        #expect(LibraryRoutes.isLibrary(.group(id: UUID())))
        #expect(!LibraryRoutes.isLibrary(.insights))
        #expect(!LibraryRoutes.isLibrary(.settings(section: .about)))
        #expect(!LibraryRoutes.isLibrary(.clipboard(filter: nil)))
    }

    @Test func titlesAndSidebarTags() {
        let work = Group(name: "Work")
        #expect(LibraryRoutes.title(.all, groups: []) == "All snippets")
        #expect(LibraryRoutes.title(.favorites, groups: []) == "Favorites")
        #expect(LibraryRoutes.title(.recent, groups: []) == "Recently used")
        #expect(LibraryRoutes.title(.disabled, groups: []) == "Turned off")
        #expect(LibraryRoutes.title(.group(id: work.id), groups: [work]) == "Work")
        #expect(LibraryRoutes.title(.group(id: UUID()), groups: []) == "Group")
        #expect(LibraryRoutes.title(.insights, groups: []) == "")
        #expect(LibraryRoutes.sidebarTag(.clipboard(filter: .all)) == .clipboard(filter: nil))
        #expect(LibraryRoutes.sidebarTag(.clipboard(filter: .pinned)) == .clipboard(filter: .pinned))
        #expect(LibraryRoutes.sidebarTag(.settings(section: .apps)) == .settings(section: nil))
        #expect(LibraryRoutes.sidebarTag(.all) == .all)
    }

    // MARK: Scope and sort

    @Test func scopeIncludesDescendantsFavoritesRecentAndDisabled() {
        let work = Group(name: "Work")
        let email = Group(name: "Email", parentID: work.id)
        let inChild = snippet("Signature", group: email.id)
        let inParent = snippet("Standup", group: work.id)
        let loose = snippet("Loose", favorite: true)
        let off = snippet("Old", enabled: false)
        let used = snippet("Used")
        let library = Library(groups: [work, email], snippets: [inChild, inParent, loose, off, used])
        let usage = [used.id: SnippetUsage(count: 1, last: 5)]
        #expect(names(SnippetListLogic.hits(SnippetListLogic.scoped(library, route: .group(id: work.id), usage: [:]), usage: [:], filter: "", sort: .name, route: .group(id: work.id))) == ["Signature", "Standup"])
        #expect(SnippetListLogic.scoped(library, route: .favorites, usage: [:]).map(\.name) == ["Loose"])
        #expect(SnippetListLogic.scoped(library, route: .disabled, usage: [:]).map(\.name) == ["Old"])
        #expect(SnippetListLogic.scoped(library, route: .recent, usage: usage).map(\.name) == ["Used"])
    }

    @Test func sortsByNameEditedUsedAndRecency() {
        let alpha = snippet("Alpha", updatedAt: Date(timeIntervalSince1970: 10))
        let bravo = snippet("Bravo", updatedAt: Date(timeIntervalSince1970: 30))
        let untitled = snippet("", updatedAt: Date(timeIntervalSince1970: 20))
        let snippets = [bravo, untitled, alpha]
        let usage = [
            alpha.id: SnippetUsage(count: 1, last: 1),
            bravo.id: SnippetUsage(count: 9, last: 9),
            untitled.id: SnippetUsage(count: 4, last: 4),
        ]
        #expect(names(SnippetListLogic.hits(snippets, usage: usage, filter: "", sort: .relevance, route: .all)) == ["Alpha", "Bravo", ""])
        #expect(names(SnippetListLogic.hits(snippets, usage: usage, filter: "", sort: .name, route: .all)) == ["Alpha", "Bravo", ""])
        #expect(names(SnippetListLogic.hits(snippets, usage: usage, filter: "", sort: .edited, route: .all)) == ["Bravo", "", "Alpha"])
        #expect(names(SnippetListLogic.hits(snippets, usage: usage, filter: "", sort: .used, route: .all)) == ["Bravo", "", "Alpha"])
        #expect(names(SnippetListLogic.hits(snippets, usage: usage, filter: "", sort: .name, route: .recent)) == ["Bravo", "", "Alpha"])
        #expect(names(SnippetListLogic.hits(snippets, usage: usage, filter: "a", sort: .name, route: .recent)) == ["Alpha", "Bravo"])
        let signal = snippet("Signal", triggers: [";sig"])
        let other = snippet("Other", content: "prefix ;sig suffix")
        #expect(names(SnippetListLogic.hits([other, signal], usage: [:], filter: ";sig", sort: .relevance, route: .all)) == ["Signal", "Other"])
    }

    @Test func equalNamesKeepTheirOrder() {
        let first = snippet("Same")
        let second = snippet("Same")
        let sorted = SnippetListLogic.stableSorted([second, first]) { $0.name < $1.name }
        #expect(sorted.map(\.id) == [second.id, first.id])
    }

    @Test func fallbackKeepsAValidSelectionAndDoesNotStealOneWhileFiltering() {
        let a = UUID(), b = UUID()
        #expect(SnippetListLogic.fallbackSelection(ids: [], selectedId: a, filter: "", route: .all, libraryIds: [a]) == nil)
        #expect(SnippetListLogic.fallbackSelection(ids: [a, b], selectedId: b, filter: "", route: .favorites, libraryIds: [a, b]) == nil)
        #expect(SnippetListLogic.fallbackSelection(ids: [a], selectedId: b, filter: "sig", route: .all, libraryIds: [a, b]) == nil)
        #expect(SnippetListLogic.fallbackSelection(ids: [a], selectedId: b, filter: "", route: .all, libraryIds: [a, b]) == nil)
        #expect(SnippetListLogic.fallbackSelection(ids: [a], selectedId: b, filter: "", route: .favorites, libraryIds: [a, b]) == a)
        #expect(SnippetListLogic.fallbackSelection(ids: [a], selectedId: nil, filter: "", route: .all, libraryIds: []) == a)
        #expect(SnippetListLogic.fallbackSelection(ids: [a], selectedId: b, filter: "", route: .all, libraryIds: [a]) == a)
    }

    @Test func draftAndEmptyCopy() {
        #expect(SnippetListLogic.draft(fromFilter: ";sig ") == SnippetDraft(triggers: [";sig"]))
        #expect(SnippetListLogic.draft(fromFilter: " Hello ") == SnippetDraft(name: "Hello"))
        #expect(SnippetListLogic.draft(fromFilter: " ;sig") == SnippetDraft(name: ";sig"))
        #expect(SnippetListLogic.emptyCopy(.favorites).title == "No favorites yet")
        #expect(SnippetListLogic.emptyCopy(.favorites).offersNew)
        #expect(SnippetListLogic.emptyCopy(.recent).title == "Nothing used yet")
        #expect(!SnippetListLogic.emptyCopy(.recent).offersNew)
        #expect(SnippetListLogic.emptyCopy(.disabled).title == "Everything is on")
        #expect(SnippetListLogic.emptyCopy(.group(id: UUID())).title == "This group is empty")
        #expect(SnippetListLogic.emptyCopy(.all).title == "Your library is empty")
        #expect(SnippetListLogic.emptyCopy(.all).body == "Snippets turn a short trigger like ;sig into text you type often.")
    }

    // MARK: Selection and bulk

    @Test func selectionMovementClampsAndExtends() {
        let ids = [UUID(), UUID(), UUID(), UUID()]
        let start = ListSelection(current: nil, rows: [], anchor: nil)
        let first = start.moved(in: ids, by: 1, extend: false)
        #expect(first.current == ids[0])
        #expect(first.rows == [ids[0]])
        let last = start.moved(in: ids, by: -1, extend: false)
        #expect(last.current == ids[3])
        let clamped = first.moved(in: ids, by: 100, extend: false)
        #expect(clamped.current == ids[3])
        let extended = first.moved(in: ids, by: 2, extend: true)
        #expect(extended.rows == Set(ids[0...2]))
        #expect(extended.anchor == ids[0])
        #expect(extended.current == ids[2])
        #expect(start.moved(in: [], by: 1, extend: false) == start)
    }

    @Test func primarySelectionPrefersTheNewlyAddedRow() {
        let a = UUID(), b = UUID(), c = UUID()
        let ids = [a, b, c]
        #expect(ListSelection.primary(old: [a], new: [a, c], previous: a, ids: ids) == c)
        #expect(ListSelection.primary(old: [], new: [b], previous: nil, ids: ids) == b)
        #expect(ListSelection.primary(old: [a, b], new: [a], previous: b, ids: ids) == a)
        #expect(ListSelection.primary(old: [a], new: [], previous: a, ids: ids) == nil)
    }

    @Test func bulkBarAppearsOnlyForAMultiSelection() {
        let on = snippet("On", favorite: true, enabled: true)
        let off = snippet("Off", favorite: false, enabled: false)
        let library = Library(snippets: [on, off])
        #expect(BulkState(selection: [], library: library) == nil)
        #expect(BulkState(selection: [on.id], library: library) == nil)
        let bulk = BulkState(selection: [on.id, off.id], library: library)
        #expect(bulk?.ids == [on.id, off.id])
        #expect(bulk?.canFavorite == true)
        #expect(bulk?.allEnabled == false)
        let starred = Library(snippets: [on, snippet("Also", favorite: true)])
        #expect(BulkState(selection: starred.snippets.map(\.id), library: starred)?.canFavorite == false)
        #expect(BulkState(selection: [on.id, on.id], library: Library(snippets: [on]))?.allEnabled == true)
    }

    @Test func groupMovesRefuseSelfAndDescendants() {
        let work = Group(name: "Work")
        let email = Group(name: "Email", parentID: work.id)
        let groups = [work, email]
        #expect(!GroupMoves.canMove(work.id, into: email.id, groups: groups))
        #expect(!GroupMoves.canMove(work.id, into: work.id, groups: groups))
        #expect(!GroupMoves.canMove(work.id, into: nil, groups: groups))
        #expect(!GroupMoves.canMove(email.id, into: work.id, groups: groups))
        #expect(GroupMoves.canMove(email.id, into: nil, groups: groups))
        #expect(!GroupMoves.canMove(UUID(), into: nil, groups: groups))
    }

    // MARK: Chrome copy

    @Test func breadcrumbsFollowTheRoute() {
        let work = Group(name: "Work")
        let email = Group(name: "Email", parentID: work.id)
        let signature = snippet("Signature", group: email.id)
        let groups = [work, email]
        #expect(LibraryBreadcrumbs.crumbs(route: .all, groups: [], snippet: nil).map(\.label) == ["Snippets", "All snippets"])
        #expect(LibraryBreadcrumbs.crumbs(route: .favorites, groups: [], snippet: nil).map(\.label) == ["Snippets", "Favorites"])
        #expect(LibraryBreadcrumbs.crumbs(route: .group(id: email.id), groups: groups, snippet: nil).map(\.label) == ["Snippets", "Work", "Email"])
        #expect(LibraryBreadcrumbs.crumbs(route: .all, groups: groups, snippet: signature).map(\.label) == ["Snippets", "Work", "Email", "Signature"])
        #expect(LibraryBreadcrumbs.crumbs(route: .all, groups: [], snippet: snippet("")).last?.label == "Untitled")
        #expect(LibraryBreadcrumbs.crumbs(route: .settings(section: nil), groups: [], snippet: nil).map(\.label) == ["Settings", "General"])
        #expect(LibraryBreadcrumbs.crumbs(route: .settings(section: .data), groups: [], snippet: nil).map(\.label) == ["Settings", "Backups & data"])
        #expect(LibraryBreadcrumbs.crumbs(route: .clipboard(filter: nil), groups: [], snippet: nil).map(\.label) == ["Clipboard", "History"])
        #expect(LibraryBreadcrumbs.crumbs(route: .clipboard(filter: .kind(.image)), groups: [], snippet: nil).map(\.label) == ["Clipboard", "Images"])
        #expect(LibraryBreadcrumbs.crumbs(route: .insights, groups: [], snippet: nil).map(\.label) == ["Insights"])
        #expect(LibraryBreadcrumbs.settingsLabel(.agents) == "AI agents")
    }

    @Test func engineHealthPrefersPauseThenTheWebPreview() {
        let blocked = EngineStatus(tap: .off, permissions: PermissionStatus(), summary: "Blocked")
        let secure = EngineStatus(paused: true, tap: .active, permissions: PermissionStatus(accessibility: true, inputMonitoring: true), secureInput: true, secureInputOwner: "1Password", summary: "Paused")
        #expect(EngineHealth(status: secure, platform: .web) == .paused)
        #expect(EngineHealth(status: blocked, platform: .web) == .ready)
        #expect(EngineHealth(status: EngineStatus(tap: .active, permissions: PermissionStatus(accessibility: true, inputMonitoring: true), secureInput: true, summary: "Secure"), platform: .macos) == .secure)
        #expect(EngineHealth(status: blocked, platform: .macos) == .blocked)
        let limited = EngineStatus(tap: .listenOnly, permissions: PermissionStatus(accessibility: false, inputMonitoring: true), summary: "Limited")
        #expect(EngineHealth(status: limited, platform: .macos) == .limited)
        let ready = EngineStatus(canExpand: true, tap: .active, permissions: PermissionStatus(accessibility: true, inputMonitoring: true), summary: "Ready")
        #expect(EngineHealth(status: ready, platform: .macos) == .ready)
        #expect(EngineHealth.blocked.opensSettings)
        #expect(EngineHealth.limited.opensSettings)
        #expect(!EngineHealth.ready.opensSettings)
        #expect(EngineHealth.blocked.showsShield)
        #expect(EngineHealth.secure.showsShield)
        #expect(!EngineHealth.paused.showsShield)
        #expect(EngineHealth.secure.detail(owner: "1Password") == "1Password has Secure Input on, so macOS blocks expansion.")
        #expect(EngineHealth.paused.label == "Paused")
    }

    @Test func toastCopy() {
        #expect(LibraryCopy.deleted([snippet("Sig")]) == "Deleted “Sig”")
        #expect(LibraryCopy.deleted([snippet("")]) == "Deleted “Untitled”")
        #expect(LibraryCopy.deleted([snippet("A"), snippet("B")]) == "Deleted 2 snippets")
        let clean = LibraryCopy.imported(ImportSummary(importedSnippets: 1, importedGroups: 0))
        #expect(clean.title == "Imported 1 snippet")
        #expect(clean.description == "A backup was saved first, just in case.")
        let warned = LibraryCopy.imported(ImportSummary(importedSnippets: 2, importedGroups: 1, warnings: [ImportWarning(message: "trimmed a trigger")]))
        #expect(warned.title == "Imported 2 snippets in 1 group")
        #expect(warned.description == "1 item imported with changes — e.g. trimmed a trigger")
        let files = LibraryCopy.exportedEspanso(1, dir: "/tmp/espanso")
        #expect(files.title == "Exported 1 Espanso file")
        #expect(files.description.contains("/tmp/espanso"))
        #expect(LibraryCopy.exportedEspanso(2, dir: "/tmp").title == "Exported 2 Espanso files")
    }

    // MARK: Command menu, sidebar memory, shortcuts

    @Test func commandMenuHidesGroupRowsUntilThereIsAQuery() {
        let work = Group(id: UUID(uuidString: "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA")!, name: "Work")
        let email = Group(name: "Email", parentID: work.id)
        let selected = snippet("Sig")
        let catalogue = CommandMenuLogic.actions(selected: selected, paused: false, theme: .dark, clipboardEnabled: true, groups: [work, email])
        #expect(catalogue.prefix(4).map(\.id) == ["fav", "en", "dup", "del"])
        #expect(catalogue.map(\.id).contains("g-\(work.id.uuidString)"))
        #expect(catalogue.map(\.label).contains("Work › Email"))
        let visible = CommandMenuLogic.visible(catalogue, query: "")
        #expect(visible.allSatisfy { !$0.id.hasPrefix("g-") && !$0.id.hasPrefix("gs-") })
        #expect(visible.contains { $0.id == "all" })
        let found = CommandMenuLogic.visible(catalogue, query: "yaml csv").map(\.id)
        #expect(found == ["imp"])
        #expect(CommandMenuLogic.visible(catalogue, query: "email").contains { $0.id == "g-\(email.id.uuidString)" })
        #expect(CommandMenuLogic.visible(catalogue, query: "nope").isEmpty)
        #expect(CommandMenuLogic.snippetLimit("") == 5)
        #expect(CommandMenuLogic.snippetLimit("  ") == 8)
        #expect(CommandMenuLogic.snippetLimit("sig") == 8)
        #expect(CommandMenuLogic.snippetHeading("") == "Frequently used")
        #expect(CommandMenuLogic.snippetHeading("sig") == "Snippets")
        #expect(catalogue.first { $0.id == "theme" }?.label == "Switch to light appearance")
        #expect(catalogue.first { $0.id == "pause" }?.label == "Pause expanding")
    }

    @Test func clipboardSidebarShowsPinnedAndOccupiedKinds() {
        let now = Date()
        func entry(_ kind: ClipKind, pinned: Bool = false) -> ClipEntry {
            ClipEntry(kind: kind, hash: kind.rawValue + (pinned ? "-p" : ""), firstCopiedAt: now, copiedAt: now, pinned: pinned)
        }
        let counts = ClipSidebar.counts([entry(.text, pinned: true), entry(.image)])
        #expect(counts["all"] == 2)
        #expect(counts["pinned"] == 1)
        #expect(counts["text"] == 1)
        #expect(counts["image"] == 1)
        #expect(counts["code"] == 0)
        let shown = ClipSidebar.visibleFilters(counts: counts, active: "code").map(\.value)
        #expect(shown == ["pinned", "text", "image", "code"])
        #expect(!shown.contains("all"))
    }

    @Test func sidebarSectionCollapseRoundTrips() {
        #expect(SidebarSections.decode(nil).isEmpty)
        #expect(SidebarSections.decode("not json").isEmpty)
        #expect(SidebarSections.decode("[\"clipboard\",\"snippets\"]") == ["clipboard", "snippets"])
        let encoded = SidebarSections.encode(["snippets", "clipboard"])
        #expect(SidebarSections.decode(encoded) == ["clipboard", "snippets"])
        #expect(encoded == "[\"clipboard\",\"snippets\"]")
    }

    @Test @MainActor func exportStampIsTheUTCDate() {
        #expect(DataActions.stamp(Date(timeIntervalSince1970: 0)) == "1970-01-01")
    }

    @Test func shortcutCatalogIncludesTheWindowCombos() {
        let combos = Set(LibraryShortcuts.sections.flatMap(\.items).map(\.combo))
        for combo in [
            "mod+k", "mod+n", "mod+shift+n", "mod+f", "mod+shift+p", "mod+,", "mod+/",
            "mod+1", "mod+2", "mod+3", "mod+4", "mod+5", "mod+l", "mod+e", "escape",
            "up", "down", "shift+down", "mod+a", "enter", "/", "mod+backspace",
            "shift+enter", "space", "mod+p", "mod+s", "mod+o",
            "mod+d", "mod+shift+f", "mod+shift+e", "mod+z",
        ] {
            #expect(combos.contains(combo), "missing \(combo)")
        }
        #expect(LibraryShortcuts.sections.map(\.title) == ["Everywhere", "Navigate", "Snippet list", "Clipboard history", "Editor"])
    }
}
