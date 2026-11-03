import Foundation

/// What kind of library delete produced a snapshot (used for redo).
public enum LibraryDeletionKind: Hashable, Sendable {
    case snippets
    case groupMoveContentsUp(groupID: UUID)
    case groupDeleteContents(groupID: UUID)
}

/// Captured records removed or relocated by one atomic library delete, for undo/redo.
public struct LibraryDeletionSnapshot: Hashable, Sendable {
    /// The original user operation.
    public let kind: LibraryDeletionKind
    /// Snippets removed by the operation.
    public let deletedSnippets: [SnippetRecord]
    /// Groups removed by the operation.
    public let deletedGroups: [GroupRecord]
    /// All per-device usage rows removed with the snippets.
    public let deletedUsage: [UsageRecord]
    /// Dedupe aliases needed to attribute usage that syncs after a deleted snippet is restored.
    public let deletedSnippetMergedIDs: [UUID: [UUID]]
    /// Snippet records before a move-contents-up delete changed their group.
    public let priorSnippets: [SnippetRecord]
    /// Group records before a move-contents-up delete changed their parent.
    public let priorGroups: [GroupRecord]

    /// Exact records after moving contents, including the actual requested destination.
    public let relocatedSnippets: [SnippetRecord]
    /// Exact child group records after the move.
    public let relocatedGroups: [GroupRecord]

    /// Captures the before and after values of one deletion.
    public init(
        kind: LibraryDeletionKind,
        deletedSnippets: [SnippetRecord] = [],
        deletedGroups: [GroupRecord] = [],
        deletedUsage: [UsageRecord] = [],
        deletedSnippetMergedIDs: [UUID: [UUID]] = [:],
        priorSnippets: [SnippetRecord] = [],
        priorGroups: [GroupRecord] = [],
        relocatedSnippets: [SnippetRecord] = [],
        relocatedGroups: [GroupRecord] = []
    ) {
        self.kind = kind
        self.deletedSnippets = deletedSnippets
        self.deletedGroups = deletedGroups
        self.deletedUsage = deletedUsage
        self.deletedSnippetMergedIDs = deletedSnippetMergedIDs
        self.priorSnippets = priorSnippets
        self.priorGroups = priorGroups
        self.relocatedSnippets = relocatedSnippets
        self.relocatedGroups = relocatedGroups
    }

    /// Whether the operation changed no records.
    public var isEmpty: Bool {
        deletedSnippets.isEmpty && deletedGroups.isEmpty && deletedUsage.isEmpty
            && priorSnippets.isEmpty && priorGroups.isEmpty
    }
}

/// Records restored by a successful undo; conflicts throw without restoring any records.
public struct LibraryRestoreResult: Hashable, Sendable {
    /// Restored or reparented snippets.
    public let restoredSnippetIDs: [UUID]
    /// Restored or reparented groups.
    public let restoredGroupIDs: [UUID]
    /// Number of restored usage rows.
    public let restoredUsageCount: Int
}

extension KudzuStoreError: LocalizedError {
    /// An actionable explanation for failed library operations.
    public var errorDescription: String? {
        switch self {
        case .deletionConflict:
            "The affected records, usage, or group contents have changed since the deletion."
        case .groupNotFound:
            "A required parent group no longer exists. Restore that group before retrying."
        case .snippetNotFound:
            "An affected snippet no longer exists."
        case .groupCycle:
            "The requested move would create a group cycle."
        }
    }
}
