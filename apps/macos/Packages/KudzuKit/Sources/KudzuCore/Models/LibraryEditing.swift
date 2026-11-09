import Foundation

/// Domain errors for pure ``Library`` mutations.
public enum LibraryEditError: LocalizedError, Equatable, Sendable, CustomStringConvertible {
    case snippetNoLongerExists
    case snippetNotFound
    case groupNotFound
    case parentGroupNotFound
    case groupCycle

    public var message: String {
        switch self {
        case .snippetNoLongerExists:
            return "That snippet no longer exists."
        case .snippetNotFound:
            return "Snippet not found"
        case .groupNotFound:
            return "Group not found"
        case .parentGroupNotFound:
            return "Parent group not found"
        case .groupCycle:
            return "A group can't be moved inside itself."
        }
    }

    public var errorDescription: String? { message }
    public var description: String { message }
}

/// Summary of the group and snippet IDs removed by ``LibraryEditing/deleteGroup(id:deleteSnippets:in:)``.
public struct GroupDeletionOutcome: Equatable, Sendable {
    public var deletedGroupIDs: Set<UUID>
    public var deletedSnippetIDs: Set<UUID>

    public init(deletedGroupIDs: Set<UUID>, deletedSnippetIDs: Set<UUID>) {
        self.deletedGroupIDs = deletedGroupIDs
        self.deletedSnippetIDs = deletedSnippetIDs
    }
}

/// Pure, deterministic mutations on a ``Library`` value, shared by production persistence and
/// in-memory preview/test backends.
public enum LibraryEditing {
    // MARK: Snippets

    /// Inserts a new snippet built from `draft` at the front of `library.snippets`.
    @discardableResult
    public static func createSnippet(
        in library: inout Library,
        draft: SnippetDraft = SnippetDraft(),
        id: UUID = UUID(),
        now: Date = Date()
    ) -> Snippet {
        let snippet = draft.snippet(id: id, now: now)
        library.snippets.insert(snippet, at: 0)
        return snippet
    }

    /// Updates an existing snippet in place, preserving its original `createdAt`, stamping
    /// `updatedAt = now`, stripping blank triggers, and leaving `formatting` as supplied.
    @discardableResult
    public static func saveSnippet(
        _ snippet: Snippet,
        in library: inout Library,
        now: Date = Date()
    ) throws -> Snippet {
        guard let index = library.snippets.firstIndex(where: { $0.id == snippet.id }) else {
            throw LibraryEditError.snippetNoLongerExists
        }
        var saved = snippet
        saved.createdAt = library.snippets[index].createdAt
        saved.updatedAt = now
        saved.triggers.removeAll { RustText.trim($0).isEmpty }
        library.snippets[index] = saved
        return saved
    }

    /// Removes every snippet whose ID is in `ids`.
    public static func deleteSnippets(
        ids: some Sequence<UUID>,
        in library: inout Library
    ) {
        let dead = Set(ids)
        library.snippets.removeAll { dead.contains($0.id) }
    }

    /// Appends `snippets` whose IDs are not already present in `library.snippets`, ignoring
    /// duplicate IDs both against existing records and within `snippets`.
    public static func restoreSnippets(
        _ snippets: [Snippet],
        in library: inout Library
    ) {
        var existing = Set(library.snippets.map(\.id))
        for snippet in snippets where existing.insert(snippet.id).inserted {
            library.snippets.append(snippet)
        }
    }

    /// Duplicates `id` immediately after the source snippet (`at + 1`), retaining the existing
    /// "name copy" behavior and choosing unused numbered triggers (`2`, `3`, …).
    @discardableResult
    public static func duplicateSnippet(
        id: UUID,
        in library: inout Library,
        newID: UUID = UUID(),
        now: Date = Date()
    ) throws -> Snippet {
        guard let index = library.snippets.firstIndex(where: { $0.id == id }) else {
            throw LibraryEditError.snippetNotFound
        }
        let source = library.snippets[index]
        var takenTriggers = Set(library.snippets.flatMap { $0.triggers.map { Data($0.lowercased().utf8) } })
        var copy = source
        copy.id = newID
        copy.name = source.name.isEmpty
            ? ""
            : "\(source.name) copy"
        copy.triggers = source.triggers.map { trigger in
            var suffix = 2
            while takenTriggers.contains(Data("\(trigger)\(suffix)".lowercased().utf8)) {
                suffix += 1
            }
            let candidate = "\(trigger)\(suffix)"
            takenTriggers.insert(Data(candidate.lowercased().utf8))
            return candidate
        }
        copy.createdAt = now
        copy.updatedAt = now
        library.snippets.insert(copy, at: index + 1)
        return copy
    }

    /// Moves matching snippets to `groupID` (`nil` = ungrouped) and updates their `updatedAt`.
    public static func moveSnippets(
        ids: some Sequence<UUID>,
        groupID: UUID?,
        in library: inout Library,
        now: Date = Date()
    ) throws {
        if let groupID, !library.groups.contains(where: { $0.id == groupID }) {
            throw LibraryEditError.parentGroupNotFound
        }
        let target = Set(ids)
        for i in library.snippets.indices where target.contains(library.snippets[i].id) {
            library.snippets[i].groupID = groupID
            library.snippets[i].updatedAt = now
        }
    }

    // MARK: Groups

    /// Appends a new group under `parentID` with the next sibling `sortIndex`.
    @discardableResult
    public static func createGroup(
        name: String,
        parentID: UUID? = nil,
        in library: inout Library,
        id: UUID = UUID()
    ) throws -> Group {
        if let parentID, !library.groups.contains(where: { $0.id == parentID }) {
            throw LibraryEditError.parentGroupNotFound
        }
        let sortIndex = library.groups
            .filter { $0.parentID == parentID }
            .map { $0.sortIndex + 1 }
            .max() ?? 0
        let group = Group(id: id, name: name, parentID: parentID, sortIndex: sortIndex)
        library.groups.append(group)
        return group
    }

    /// Whether making `parentID` the parent of `id` would place a group inside itself or a cycle.
    public static func wouldCycle(
        _ groups: [Group],
        id: UUID,
        parentID: UUID?
    ) -> Bool {
        var current = parentID
        var seen = Set<UUID>()
        while let parent = current {
            if parent == id || !seen.insert(parent).inserted {
                return true
            }
            current = groups.first { $0.id == parent }?.parentID
        }
        return false
    }

    /// Replaces `group` in `library.groups`, refusing missing groups, missing parents, or cycles.
    @discardableResult
    public static func saveGroup(
        _ group: Group,
        in library: inout Library
    ) throws -> Group {
        guard let index = library.groups.firstIndex(where: { $0.id == group.id }) else {
            throw LibraryEditError.groupNotFound
        }
        if wouldCycle(library.groups, id: group.id, parentID: group.parentID) {
            throw LibraryEditError.groupCycle
        }
        if let parentID = group.parentID, !library.groups.contains(where: { $0.id == parentID }) {
            throw LibraryEditError.parentGroupNotFound
        }
        library.groups[index] = group
        return group
    }

    /// Moves `id` under `parentID` at sibling position `index` (clamped), deterministically
    /// reindexing destination siblings (and remaining source siblings when changing parents).
    public static func moveGroup(
        id: UUID,
        parentID: UUID?,
        index: Int,
        in library: inout Library
    ) throws {
        guard let movingIndex = library.groups.firstIndex(where: { $0.id == id }) else {
            throw LibraryEditError.groupNotFound
        }
        if wouldCycle(library.groups, id: id, parentID: parentID) {
            throw LibraryEditError.groupCycle
        }
        if let parentID, !library.groups.contains(where: { $0.id == parentID }) {
            throw LibraryEditError.parentGroupNotFound
        }

        let oldParentID = library.groups[movingIndex].parentID
        var siblings = orderedSiblingIDs(parentID: parentID, excluding: id, in: library.groups)
        let clampedIndex = min(max(index, 0), siblings.count)
        siblings.insert(id, at: clampedIndex)
        let newPositions = Dictionary(uniqueKeysWithValues: siblings.enumerated().map { ($1, $0) })

        let oldPositions: [UUID: Int]
        if oldParentID != parentID {
            let remainingOld = orderedSiblingIDs(parentID: oldParentID, excluding: id, in: library.groups)
            oldPositions = Dictionary(uniqueKeysWithValues: remainingOld.enumerated().map { ($1, $0) })
        } else {
            oldPositions = [:]
        }

        for i in library.groups.indices {
            let groupID = library.groups[i].id
            if let pos = newPositions[groupID] {
                library.groups[i].sortIndex = pos
                if groupID == id {
                    library.groups[i].parentID = parentID
                }
            } else if let pos = oldPositions[groupID] {
                library.groups[i].sortIndex = pos
            }
        }
    }

    /// Deletes `id` and all its descendant groups. When `deleteSnippets` is true, snippets in
    /// any deleted group are removed; otherwise their `groupID` is cleared to `nil`.
    @discardableResult
    public static func deleteGroup(
        id: UUID,
        deleteSnippets: Bool,
        in library: inout Library
    ) -> GroupDeletionOutcome {
        let doomed = Groups.descendantIds(library.groups, id: id)
        library.groups.removeAll { doomed.contains($0.id) }
        var deletedSnippetIDs = Set<UUID>()
        if deleteSnippets {
            library.snippets.removeAll { snippet in
                guard let groupID = snippet.groupID, doomed.contains(groupID) else { return false }
                deletedSnippetIDs.insert(snippet.id)
                return true
            }
        } else {
            for i in library.snippets.indices {
                if let groupID = library.snippets[i].groupID, doomed.contains(groupID) {
                    library.snippets[i].groupID = nil
                }
            }
        }
        return GroupDeletionOutcome(deletedGroupIDs: doomed, deletedSnippetIDs: deletedSnippetIDs)
    }

    // MARK: Internal

    private static func orderedSiblingIDs(
        parentID: UUID?,
        excluding excludedID: UUID?,
        in groups: [Group]
    ) -> [UUID] {
        groups.enumerated()
            .filter { $0.element.parentID == parentID && $0.element.id != excludedID }
            .sorted { lhs, rhs in
                if lhs.element.sortIndex != rhs.element.sortIndex {
                    return lhs.element.sortIndex < rhs.element.sortIndex
                }
                let nameOrder = lhs.element.name.localizedCompare(rhs.element.name)
                if nameOrder != .orderedSame {
                    return nameOrder == .orderedAscending
                }
                return lhs.offset < rhs.offset
            }
            .map(\.element.id)
    }
}
