import Foundation
import KudzuCore
import Observation

/// What happens to a group's contents when it is deleted.
enum GroupDeletion {
    /// Snippets and subgroups move to the deleted group's parent.
    case moveContentsUp
    /// Snippets and subgroups are deleted with it.
    case deleteContents
}

/// A field the editor should focus once it appears (after creating a snippet).
enum EditorFocusTarget {
    case name
    case trigger
}

/// Value snapshot of the library plus derived data, and every mutation the main window performs.
///
/// Views render from the snapshot (`KudzuLibrary` values), never from SwiftData objects. The
/// snapshot is refreshed by `reload()` when `store.changes` fires, and patched in place for editor
/// autosaves so a keystroke never re-fetches the whole library.
@MainActor @Observable
final class LibraryModel {
    @ObservationIgnored let store: KudzuStore

    private(set) var isLoaded = false
    /// Bumped whenever the snapshot changes; cheap to observe with `onChange`.
    private(set) var revision = 0

    private(set) var groups: [GroupRecord] = []
    private(set) var snippets: [SnippetRecord] = []
    private(set) var snippetsByID: [UUID: SnippetRecord] = [:]
    private(set) var contentByID: [UUID: String] = [:]
    private(set) var tree = GroupTree(groups: [])
    /// Snippets per group including subgroups.
    private(set) var groupCounts: [UUID: Int] = [:]
    private(set) var searchIndex = SnippetSearchIndex()
    private(set) var stats: [UUID: SnippetStats] = [:]
    private(set) var summary = StatsSummary(perSnippet: [], totalExpansions: 0, totalCharsSaved: 0, estimatedTimeSaved: 0)
    private(set) var conflicts: [TriggerConflict] = []
    private(set) var typingSpeedWPM = 40
    private(set) var defaultTriggerPrefix = ";"

    /// Non-blocking error banner text; set by failed operations.
    var errorMessage: String?
    /// Set when a new snippet should grab focus in the editor; consumed by the editor.
    var pendingFocus: EditorFocusTarget?

    init(store: KudzuStore) {
        self.store = store
    }

    // MARK: - Loading

    /// Loads the library once; later refreshes go through `reload()`.
    func ensureLoaded() {
        if !isLoaded { reload() }
    }

    /// Re-reads the library from the store.
    func reload() {
        do {
            let library = try store.library()
            let settings = try store.syncedSettings()
            ingest(library, wpm: settings.typingSpeedWPM, prefix: settings.defaultTriggerPrefix)
        } catch {
            report("Couldn't load the library", error)
        }
    }

    /// Follows `store.changes` for the lifetime of the calling task. Call once per window from `.task`.
    func observeChanges() async {
        // Subscribe before reloading so a save landing in between can't be missed.
        let changes = store.changes
        reload()
        for await _ in changes {
            if Task.isCancelled { break }
            reload()
        }
    }

    private func ingest(_ library: KudzuLibrary, wpm: Int, prefix: String) {
        var conflictInputsChanged = false
        var changed = !isLoaded

        if groups != library.groups {
            groups = library.groups
            tree = GroupTree(groups: library.groups)
            conflictInputsChanged = true
            changed = true
        }
        if snippets != library.snippets {
            let old = snippetsByID
            let new = Dictionary(library.snippets.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
            for snippet in library.snippets where old[snippet.id] != snippet {
                searchIndex.update(snippet)
                if let before = old[snippet.id] {
                    if Self.affectsConflicts(before, snippet) { conflictInputsChanged = true }
                } else {
                    conflictInputsChanged = true
                }
            }
            for id in old.keys where new[id] == nil {
                searchIndex.remove(id)
                conflictInputsChanged = true
            }
            snippets = library.snippets
            snippetsByID = new
            contentByID = new.mapValues(\.content)
            changed = true
        }
        if changed { groupCounts = tree.recursiveSnippetCounts(snippets) }
        if conflictInputsChanged || !isLoaded {
            conflicts = TriggerConflicts.find(snippets: snippets, groups: groups)
        }

        let newSummary = StatsCalculator.summary(usage: library.usage, snippets: library.snippets, typingSpeedWPM: wpm)
        if newSummary != summary {
            summary = newSummary
            stats = Dictionary(newSummary.perSnippet.map { ($0.snippetID, $0) }, uniquingKeysWith: { first, _ in first })
            changed = true
        }
        if typingSpeedWPM != wpm { typingSpeedWPM = wpm; changed = true }
        if defaultTriggerPrefix != prefix { defaultTriggerPrefix = prefix }

        isLoaded = true
        if changed { revision += 1 }
    }

    private static func affectsConflicts(_ a: SnippetRecord, _ b: SnippetRecord) -> Bool {
        a.triggers != b.triggers || a.fireMode != b.fireMode || a.caseMode != b.caseMode
            || a.rule != b.rule || a.groupID != b.groupID || a.isEnabled != b.isEnabled
    }

    private func report(_ prefix: String, _ error: Error) {
        errorMessage = "\(prefix): \(error.localizedDescription)"
    }

    /// Runs a store mutation, reports failure in the banner, and refreshes the snapshot either way.
    @discardableResult
    private func perform(_ failure: String, _ body: () throws -> Void) -> Bool {
        defer { reload() }
        do {
            try body()
            return true
        } catch {
            report(failure, error)
            return false
        }
    }

    // MARK: - Queries

    /// Snippets for a sidebar scope, filtered by `query` and ordered by `sort`.
    ///
    /// Recently Used ignores `sort`: it is always the 20 most recently used, newest first.
    func snippets(in scope: SidebarScope, query: String, sort: SnippetSortOrder) -> [SnippetRecord] {
        let base: [SnippetRecord]
        var order = sort
        switch scope {
        case .all:
            base = snippets
        case .favorites:
            base = snippets.filter(\.isFavorite)
        case .recent:
            base = recentSnippets()
            order = .recentlyUsed
        case .group(let id):
            let ids = tree.descendantIDs(of: id)
            base = snippets.filter { $0.groupID.map(ids.contains) ?? false }
        case .stats:
            return []
        }
        let filtered = searchIndex.filter(base, query: query)
        return searchIndex.sorted(filtered, by: order, stats: stats)
    }

    /// The most recently used snippets, newest first.
    func recentSnippets(limit: Int = 20) -> [SnippetRecord] {
        snippets
            .compactMap { snippet in stats[snippet.id]?.lastUsedAt.map { (snippet, $0) } }
            .sorted { $0.1 > $1.1 }
            .prefix(limit)
            .map(\.0)
    }

    /// Breadcrumb like "Work / Email" for a snippet's group, or nil when ungrouped.
    func groupPath(of snippet: SnippetRecord) -> String? {
        guard let id = snippet.groupID, tree.group(id) != nil else { return nil }
        return tree.path(to: id).joined(separator: " / ")
    }

    /// Whether the snippet itself and every group above it are enabled.
    func isActive(_ snippet: SnippetRecord) -> Bool {
        guard snippet.isEnabled else { return false }
        guard let groupID = snippet.groupID, tree.group(groupID) != nil else { return true }
        return tree.isEffectivelyEnabled(groupID)
    }

    /// Trigger conflicts that involve the snippet.
    func conflicts(for id: UUID) -> [TriggerConflict] {
        conflicts.filter { $0.snippetID == id || $0.otherSnippetID == id }
    }

    // MARK: - Snippet mutations

    /// Saves an editor draft. Patches the snapshot instead of re-fetching, since this runs after every pause in typing.
    func saveEdited(_ snippet: SnippetRecord) -> Bool {
        do {
            try store.upsert(snippet)
        } catch {
            report("Couldn't save “\(snippet.name)”", error)
            return false
        }
        if let index = snippets.firstIndex(where: { $0.id == snippet.id }) {
            let before = snippets[index]
            snippets[index] = snippet
            if Self.affectsConflicts(before, snippet) {
                conflicts = TriggerConflicts.find(snippets: snippets, groups: groups)
            }
        }
        snippetsByID[snippet.id] = snippet
        contentByID[snippet.id] = snippet.content
        searchIndex.update(snippet)
        revision += 1
        return true
    }

    /// Creates a snippet with a unique placeholder name and a trigger seeded from the default prefix.
    @discardableResult
    func createSnippet(in groupID: UUID?, content: String = "", nameHint: String? = nil) -> UUID? {
        ensureLoaded()
        let name = UniqueName.make(base: nameHint ?? "New Snippet", existing: snippets.map(\.name))
        let trigger = UniqueName.makeTrigger(base: defaultTriggerPrefix + "snippet", existing: snippets.flatMap(\.triggers))
        let parent = groupID.flatMap { tree.group($0) != nil ? $0 : nil }
        let snippet = SnippetRecord(name: name, triggers: [trigger], content: content, groupID: parent)
        let created = perform("Couldn't create the snippet") { try store.upsert(snippet) }
        return created ? snippet.id : nil
    }

    /// Copies snippets, renaming them and numbering their triggers so the copies don't conflict.
    func duplicate(_ ids: [UUID]) -> [UUID] {
        var names = snippets.map(\.name)
        var triggers = snippets.flatMap(\.triggers)
        var created: [UUID] = []
        perform("Couldn't duplicate") {
            for id in ids {
                guard var copy = snippetsByID[id] else { continue }
                copy.id = UUID()
                copy.name = UniqueName.make(base: "\(copy.name) Copy", existing: names)
                copy.triggers = copy.triggers.map { UniqueName.makeTrigger(base: $0, existing: triggers) }
                copy.createdAt = .now
                copy.updatedAt = .now
                names.append(copy.name)
                triggers += copy.triggers
                try store.upsert(copy)
                created.append(copy.id)
            }
        }
        return created
    }

    /// Deletes snippets and their usage.
    func deleteSnippets(_ ids: [UUID]) {
        perform("Couldn't delete") {
            for id in ids { try store.deleteSnippet(id: id) }
        }
    }

    /// Moves snippets into a group (nil = no group).
    func moveSnippets(_ ids: [UUID], to groupID: UUID?) {
        perform("Couldn't move") {
            for id in ids {
                guard var snippet = snippetsByID[id], snippet.groupID != groupID else { continue }
                snippet.groupID = groupID
                snippet.updatedAt = .now
                try store.upsert(snippet)
            }
        }
    }

    /// Applies one edit to several snippets in a single pass.
    func update(_ ids: [UUID], _ body: (inout SnippetRecord) -> Void) {
        perform("Couldn't update") {
            for id in ids {
                guard var snippet = snippetsByID[id] else { continue }
                let before = snippet
                body(&snippet)
                guard snippet != before else { continue }
                snippet.updatedAt = .now
                try store.upsert(snippet)
            }
        }
    }

    // MARK: - Group mutations

    /// Creates a group at the end of `parentID`'s children and returns its ID.
    @discardableResult
    func createGroup(under parentID: UUID?, name: String = "New Group") -> UUID? {
        ensureLoaded()
        let parent = parentID.flatMap { tree.group($0) != nil ? $0 : nil }
        let siblings = tree.children(of: parent).map(\.name)
        let group = GroupRecord(name: UniqueName.make(base: name, existing: siblings), parentID: parent,
                                sortIndex: tree.nextSortIndex(under: parent))
        let created = perform("Couldn't create the group") { try store.upsert(group) }
        return created ? group.id : nil
    }

    /// Renames a group; blank names are ignored.
    func renameGroup(_ id: UUID, to name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, var group = tree.group(id), group.name != trimmed else { return }
        group.name = trimmed
        perform("Couldn't rename the group") { try store.upsert(group) }
    }

    /// Enables or disables a group (and so everything in it).
    func setGroupEnabled(_ id: UUID, _ enabled: Bool) {
        guard var group = tree.group(id), group.isEnabled != enabled else { return }
        group.isEnabled = enabled
        perform("Couldn't update the group") { try store.upsert(group) }
    }

    /// Deletes a group, either moving its contents up a level or deleting them too.
    func deleteGroup(_ id: UUID, _ strategy: GroupDeletion) {
        guard tree.group(id) != nil else { return }
        perform("Couldn't delete the group") {
            switch strategy {
            case .moveContentsUp:
                try store.deleteGroup(id: id, moveChildrenTo: tree.parent(of: id))
            case .deleteContents:
                let doomed = tree.descendantIDs(of: id)
                for snippet in snippets where snippet.groupID.map(doomed.contains) ?? false {
                    try store.deleteSnippet(id: snippet.id)
                }
                // Deepest first, so no group is deleted while it still has children.
                let ordered = doomed.sorted { tree.ancestorIDs(of: $0).count > tree.ancestorIDs(of: $1).count }
                for groupID in ordered { try store.deleteGroup(id: groupID, moveChildrenTo: nil) }
            }
        }
    }

    /// Moves a group under `parentID` (nil = top level) at `index` among its new siblings. Cycles are refused.
    @discardableResult
    func moveGroup(_ id: UUID, toParent parentID: UUID?, at index: Int?) -> Bool {
        guard let changes = tree.reparent(id, toParent: parentID, at: index) else { return false }
        guard !changes.isEmpty else { return true }
        return perform("Couldn't move the group") {
            for group in changes { try store.upsert(group) }
        }
    }
}
