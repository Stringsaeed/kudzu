import AppKit
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
/// snapshot is refreshed by `reload()` when `store.changes` fires. Drafts are retained here so
/// navigation and commands share the same unsaved edits.
@MainActor @Observable
final class LibraryModel {
    @ObservationIgnored let store: KudzuStore
    private var editors: [UUID: SnippetEditorModel] = [:]
    @ObservationIgnored private let readSnippet: (UUID) throws -> SnippetRecord?
    @ObservationIgnored private let writeSnippet: (SnippetRecord) throws -> Void
    @ObservationIgnored private let writeSnippets: ([SnippetRecord]) throws -> Void
    @ObservationIgnored private let removeSnippets: ([UUID]) throws -> LibraryDeletionSnapshot
    @ObservationIgnored private let deleteGroupContents: (UUID) throws -> LibraryDeletionSnapshot
    @ObservationIgnored private let deleteGroupMovingContentsUp: (UUID, UUID?) throws -> LibraryDeletionSnapshot
    /// Window undo manager for library deletes; nil when no main window is attached.
    @ObservationIgnored weak var undoManager: UndoManager?

    enum DeletionDirection {
        case restore, reapply
        var inverse: Self { self == .restore ? .reapply : .restore }
        var retryTitle: String { self == .restore ? "Retry Restore" : "Retry Delete" }
    }

    struct DeletionRetry: Identifiable {
        let id = UUID()
        let snapshot: LibraryDeletionSnapshot
        let direction: DeletionDirection
        let message: String
        var title: String { direction.retryTitle }
    }

    /// Failed native undo/redo commands remain recoverable after UndoManager consumes them.
    private(set) var deletionRetries: [DeletionRetry] = []

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

    init(
        store: KudzuStore,
        readSnippet: ((UUID) throws -> SnippetRecord?)? = nil,
        writeSnippet: ((SnippetRecord) throws -> Void)? = nil,
        writeSnippets: (([SnippetRecord]) throws -> Void)? = nil,
        removeSnippets: (([UUID]) throws -> LibraryDeletionSnapshot)? = nil,
        deleteGroupContents: ((UUID) throws -> LibraryDeletionSnapshot)? = nil,
        deleteGroupMovingContentsUp: ((UUID, UUID?) throws -> LibraryDeletionSnapshot)? = nil
    ) {
        self.store = store
        self.readSnippet = readSnippet ?? { id in try store.library().snippets.first { $0.id == id } }
        self.writeSnippet = writeSnippet ?? { try store.upsert($0) }
        self.writeSnippets = writeSnippets ?? { try store.upsert(snippets: $0) }
        self.removeSnippets = removeSnippets ?? { try store.deleteSnippets(ids: $0) }
        self.deleteGroupContents = deleteGroupContents ?? { try store.deleteGroupContents(rootID: $0) }
        self.deleteGroupMovingContentsUp = deleteGroupMovingContentsUp ?? { id, parent in
            try store.deleteGroupMovingContentsUp(id: id, moveChildrenTo: parent)
        }
    }

    /// Reuses drafts across navigation, including drafts whose last save failed.
    func editor(for snippet: SnippetRecord) -> SnippetEditorModel {
        if let editor = editors[snippet.id] { return editor }
        return SnippetEditorModel(snippet: snippet, library: self)
    }

    /// Registers a draft before any library command can act on its snippet.
    func retainEditor(_ editor: SnippetEditorModel) {
        editors[editor.draft.id] = editor
    }

    /// Releases clean editors on navigation while retaining every unsaved draft.
    func releaseEditor(_ editor: SnippetEditorModel) {
        let id = editor.draft.id
        if editors[id] === editor, !editor.isDirty { editors[id] = nil }
    }

    /// Failed drafts stay reachable even when a remote deletion removed the list row.
    var recoveryDrafts: [SnippetRecord] {
        editors.values.filter { $0.isDirty && $0.saveError != nil }.map(\.draft)
            .sorted { $0.id.uuidString < $1.id.uuidString }
    }

    /// The latest draft when one exists, otherwise the stored snapshot.
    func editableSnippet(_ id: UUID) -> SnippetRecord? {
        if let editor = editors[id], editor.isDirty { return editor.draft }
        return snippetsByID[id]
    }

    /// Fetches current storage immediately before saving or discarding a draft.
    func storedSnippet(_ id: UUID) throws -> SnippetRecord? {
        try readSnippet(id)
    }

    /// Saves every retained draft, allowing quit only when none remain unsaved.
    func savePendingDrafts() -> Bool {
        let results = Array(editors.values).map { $0.flush() }
        return results.allSatisfy { $0 }
    }

    /// Prepares a complete snapshot for user-requested backups, exports, imports and restoration.
    func libraryForTransfer() throws -> KudzuLibrary {
        guard savePendingDrafts() else { throw TransferError.unsavedEdits }
        return try store.library()
    }

    private enum TransferError: LocalizedError {
        case unsavedEdits
        var errorDescription: String? {
            "Some edits couldn't be saved. Close Settings to review or discard them, then try again."
        }
    }

    private func prepare(_ ids: [UUID]) -> Bool {
        // Check every target, retaining all failures, before starting the command.
        let results = ids.compactMap { editors[$0] }.map { $0.flush() }
        guard results.allSatisfy({ $0 }) else { return false }
        return reload()
    }

    private func finalizeDeletion(_ snapshot: LibraryDeletionSnapshot) {
        for id in snapshot.deletedSnippets.map(\.id) {
            editors[id]?.didDelete()
            editors[id] = nil
        }
        registerLibraryUndo(for: snapshot)
    }

    // MARK: - Loading

    /// Loads the library once; later refreshes go through `reload()`.
    func ensureLoaded() {
        if !isLoaded { reload() }
    }

    /// Re-reads the library from the store.
    @discardableResult
    func reload() -> Bool {
        do {
            let library = try store.library()
            let settings = try store.syncedSettings()
            ingest(library, wpm: settings.typingSpeedWPM, prefix: settings.defaultTriggerPrefix)
            return true
        } catch {
            report("Couldn't load the library", error)
            return false
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
        for (id, editor) in editors {
            if let stored = snippetsByID[id] { editor.adoptExternal(stored) }
        }
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
            // SwiftData leaves failed mutations in the context; don't let a later save commit them.
            store.context.rollback()
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

    /// Commits a draft only if its changed fields still agree with the current stored baseline.
    func saveEdited(_ snippet: SnippetRecord, baseline: SnippetRecord) throws -> SnippetRecord {
        var merged = try SnippetDraft.merge(snippet, baseline: baseline, latest: storedSnippet(snippet.id))
        merged.updatedAt = .now
        do {
            try writeSnippet(merged)
        } catch {
            store.context.rollback()
            throw error
        }
        // Read back store normalization (for example a group removed on another device).
        guard let saved = try storedSnippet(snippet.id) else { throw SnippetDraft.Conflict.deleted }
        reload()
        return saved
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
        guard prepare(ids) else { return [] }
        var names = snippets.map(\.name)
        var triggers = snippets.flatMap(\.triggers)
        var copies: [SnippetRecord] = []
        let succeeded = perform("Couldn't duplicate") {
            for id in ids {
                guard var copy = snippetsByID[id] else { continue }
                copy.id = UUID()
                copy.name = UniqueName.make(base: "\(copy.name) Copy", existing: names)
                copy.triggers = copy.triggers.map { UniqueName.makeTrigger(base: $0, existing: triggers) }
                copy.createdAt = .now
                copy.updatedAt = .now
                names.append(copy.name)
                triggers += copy.triggers
                copies.append(copy)
            }
            try writeSnippets(copies)
        }
        return succeeded ? copies.map(\.id) : []
    }

    /// Deletes snippets and their usage.
    func deleteSnippets(_ ids: [UUID]) {
        guard prepare(ids) else { return }
        perform("Couldn't delete") {
            let snapshot = try removeSnippets(ids)
            finalizeDeletion(snapshot)
        }
    }

    /// Moves snippets into a group (nil = no group).
    func moveSnippets(_ ids: [UUID], to groupID: UUID?) {
        update(ids) { $0.groupID = groupID }
    }

    /// Applies one edit to several snippets in a single pass.
    func update(_ ids: [UUID], _ body: (inout SnippetRecord) -> Void) {
        guard prepare(ids) else { return }
        perform("Couldn't update") {
            var changed: [SnippetRecord] = []
            for id in ids {
                guard var snippet = snippetsByID[id] else { continue }
                let before = snippet
                body(&snippet)
                guard snippet != before else { continue }
                snippet.updatedAt = .now
                changed.append(snippet)
            }
            try writeSnippets(changed)
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

    /// Updates a group's app rule and insertion override.
    func setGroupRules(_ id: UUID, rule: AppRule?, insertion: InsertionMethod?) {
        guard var group = tree.group(id), group.rule != rule || group.insertion != insertion else { return }
        group.rule = rule
        group.insertion = insertion
        perform("Couldn't update the group") { try store.upsert(group) }
    }

    /// Deletes a group, either moving its contents up a level or deleting them too.
    func deleteGroup(_ id: UUID, _ strategy: GroupDeletion) {
        guard reload(), tree.group(id) != nil else { return }
        let descendants = tree.descendantIDs(of: id)
        let affected = editors.values.filter { editor in
            editor.draft.groupID.map(descendants.contains) == true
                || snippetsByID[editor.draft.id]?.groupID.map(descendants.contains) == true
        }.map { $0.draft.id }
        guard prepare(affected) else { return }
        perform("Couldn't delete the group") {
            let snapshot: LibraryDeletionSnapshot
            switch strategy {
            case .moveContentsUp:
                snapshot = try deleteGroupMovingContentsUp(id, tree.parent(of: id))
            case .deleteContents:
                snapshot = try deleteGroupContents(id)
            }
            finalizeDeletion(snapshot)
        }
    }

    // MARK: - Library undo

    private func registerLibraryUndo(for snapshot: LibraryDeletionSnapshot) {
        registerLibraryOperation(snapshot, direction: .restore, actionName: undoActionName(for: snapshot))
    }

    private func registerLibraryOperation(
        _ snapshot: LibraryDeletionSnapshot, direction: DeletionDirection, actionName: String
    ) {
        guard !snapshot.isEmpty, let undoManager else { return }
        let needsGroup = !undoManager.isUndoing && !undoManager.isRedoing && undoManager.groupingLevel == 0
        if needsGroup { undoManager.beginUndoGrouping() }
        defer { if needsGroup { undoManager.endUndoGrouping() } }
        undoManager.registerUndo(withTarget: self) { target in
            target.applyLibraryOperation(snapshot, direction: direction, actionName: actionName)
        }
        undoManager.setActionName(actionName)
    }

    /// Retries a refused operation as a new user action, itself undoable after it succeeds.
    func retryLibraryDeletion(_ id: UUID) {
        guard let retry = deletionRetries.first(where: { $0.id == id }) else { return }
        let name = retry.direction == .restore ? "Restore Deleted Items" : undoActionName(for: retry.snapshot)
        applyLibraryOperation(retry.snapshot, direction: retry.direction, actionName: name, retryID: id)
    }

    private func applyLibraryOperation(
        _ snapshot: LibraryDeletionSnapshot, direction: DeletionDirection, actionName: String, retryID: UUID? = nil
    ) {
        defer { reload() }
        let affected = snapshot.deletedSnippets + snapshot.priorSnippets
        do {
            // Undo and redo are not consent to discard a later draft. Keep it intact for explicit Save/Discard.
            guard !affected.contains(where: { editors[$0.id]?.isDirty == true }) else {
                throw DeletionDraftError.pendingChanges
            }
            switch direction {
            case .restore:
                try store.restoreDeletion(snapshot)
            case .reapply:
                try store.reapplyDeletion(snapshot)
                for id in snapshot.deletedSnippets.map(\.id) {
                    editors[id]?.didDelete()
                    editors[id] = nil
                }
            }
        } catch {
            let message = "No items were changed. \(error.localizedDescription) Review the affected items, then choose \(direction.retryTitle)."
            // Registering an inverse here would falsely advance a failed native undo to redo.
            if let retryID { deletionRetries.removeAll { $0.id == retryID } }
            deletionRetries.append(DeletionRetry(snapshot: snapshot, direction: direction, message: message))
            errorMessage = message
            return
        }
        if let retryID { deletionRetries.removeAll { $0.id == retryID } }
        errorMessage = nil
        registerLibraryOperation(snapshot, direction: direction.inverse, actionName: actionName)
    }

    private enum DeletionDraftError: LocalizedError {
        case pendingChanges
        var errorDescription: String? {
            "An affected snippet has unsaved edits. Save or use Discard Changes first. Saved edits may still conflict with the original deletion."
        }
    }

    private func undoActionName(for snapshot: LibraryDeletionSnapshot) -> String {
        switch snapshot.kind {
        case .snippets:
            let count = snapshot.deletedSnippets.count
            return count == 1 ? "Delete Snippet" : "Delete \(count) Snippets"
        case .groupMoveContentsUp, .groupDeleteContents:
            return snapshot.deletedGroups.first.map { "Delete Group “\($0.name)”" } ?? "Delete Group"
        }
    }

    /// Moves a group under `parentID` (nil = top level) at `index` among its new siblings. Cycles are refused.
    @discardableResult
    func moveGroup(_ id: UUID, toParent parentID: UUID?, at index: Int?) -> Bool {
        guard let changes = tree.reparent(id, toParent: parentID, at: index) else { return false }
        guard !changes.isEmpty else { return true }
        return perform("Couldn't move the group") {
            try store.upsert(groups: changes)
        }
    }
}
