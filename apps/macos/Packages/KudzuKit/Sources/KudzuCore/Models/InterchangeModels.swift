import Foundation

// Serializable results of `kudzu-core/src/interchange` (backups, imports, dedupe) and the
// `EspansoDetection` of `src-tauri/src/commands.rs`. The importers live in `Interchange/`.

// MARK: - Backups

/// Why a backup snapshot was created.
public enum BackupReason: String, Codable, Hashable, Sendable, CaseIterable {
    case daily
    case preImport
    /// Safety copy of the current library taken before a restore replaces it.
    case preRestore
    case manual
    /// Taken on the first launch of a new app version, before anything migrates.
    case preUpdate
}

/// A stored backup file.
public struct BackupInfo: Codable, Hashable, Sendable {
    public var path: String
    public var filename: String
    /// Whole-second ISO-8601 (`iso8601`).
    public var createdAt: Date
    public var reason: BackupReason

    public init(path: String, filename: String, createdAt: Date, reason: BackupReason) {
        self.path = path
        self.filename = filename
        self.createdAt = createdAt
        self.reason = reason
    }

    private enum CodingKeys: String, CodingKey { case path, filename, createdAt, reason }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        path = try c.decode(String.self, forKey: .path)
        filename = try c.decode(String.self, forKey: .filename)
        createdAt = try c.decodeDate(forKey: .createdAt)
        reason = try c.decode(BackupReason.self, forKey: .reason)
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(path, forKey: .path)
        try c.encode(filename, forKey: .filename)
        try c.encodeWholeSeconds(createdAt, forKey: .createdAt)
        try c.encode(reason, forKey: .reason)
    }
}

// MARK: - Imports

/// A warning emitted during an import for an unsupported or degraded feature.
public struct ImportWarning: Codable, Hashable, Sendable {
    /// Path of the source file relative to the imported directory.
    public var file: String
    /// 1-based line in `file`, when known.
    public var line: Int?
    /// Index of the match in the file's `matches:` list, when the warning is about one match.
    public var matchIndex: Int?
    public var trigger: String?
    public var message: String

    public init(file: String = "", line: Int? = nil, matchIndex: Int? = nil, trigger: String? = nil, message: String) {
        self.file = file
        self.line = line
        self.matchIndex = matchIndex
        self.trigger = trigger
        self.message = message
    }

    private enum CodingKeys: String, CodingKey { case file, line, matchIndex, trigger, message }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        file = try c.decode(String.self, forKey: .file, default: "")
        line = try c.decodeIfPresent(Int.self, forKey: .line)
        matchIndex = try c.decodeIfPresent(Int.self, forKey: .matchIndex)
        trigger = try c.decodeIfPresent(String.self, forKey: .trigger)
        message = try c.decode(String.self, forKey: .message)
    }
}

/// Import warnings sharing one message.
public struct WarningGroup: Codable, Hashable, Sendable {
    public var message: String
    public var count: Int
    public var warnings: [ImportWarning]

    public init(message: String, count: Int, warnings: [ImportWarning]) {
        self.message = message
        self.count = count
        self.warnings = warnings
    }
}

/// Two or more surviving snippets share a trigger but expand to different content.
public struct DedupeConflict: Codable, Hashable, Sendable {
    /// The shared trigger, lowercased.
    public var trigger: String
    /// The snippets claiming it, sorted.
    public var snippetIds: [UUID]

    public init(trigger: String, snippetIds: [UUID]) {
        self.trigger = trigger
        self.snippetIds = snippetIds
    }

    private enum CodingKeys: String, CodingKey { case trigger, snippetIds }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(trigger, forKey: .trigger)
        try c.encodeUUIDs(snippetIds, forKey: .snippetIds)
    }
}

/// Counts of what a dedupe pass changed, plus conflicts for the UI to surface.
public struct DedupeReport: Codable, Hashable, Sendable {
    public var mergedGroups: Int
    public var mergedSnippets: Int
    /// Number of (snippet, device) usage rows rewritten.
    public var mergedUsageRows: Int
    public var conflicts: [DedupeConflict]

    public init(mergedGroups: Int = 0, mergedSnippets: Int = 0, mergedUsageRows: Int = 0, conflicts: [DedupeConflict] = []) {
        self.mergedGroups = mergedGroups
        self.mergedSnippets = mergedSnippets
        self.mergedUsageRows = mergedUsageRows
        self.conflicts = conflicts
    }

    /// True when nothing was merged (conflicts may still exist).
    public var isEmpty: Bool { mergedGroups == 0 && mergedSnippets == 0 && mergedUsageRows == 0 }

    private enum CodingKeys: String, CodingKey { case mergedGroups, mergedSnippets, mergedUsageRows, conflicts }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        mergedGroups = try c.decode(Int.self, forKey: .mergedGroups, default: 0)
        mergedSnippets = try c.decode(Int.self, forKey: .mergedSnippets, default: 0)
        mergedUsageRows = try c.decode(Int.self, forKey: .mergedUsageRows, default: 0)
        conflicts = try c.decode([DedupeConflict].self, forKey: .conflicts, default: [])
    }
}

/// A completed import (Espanso, Raycast, TextExpander or Kudzu JSON).
///
/// The Rust summary always carries `backup`, `dedupeReport` and `groupedWarnings`; they are
/// optional here because the TypeScript type (and the mock) leave them out.
public struct ImportSummary: Codable, Hashable, Sendable {
    /// Snippets in the source, before deduplication.
    public var importedSnippets: Int
    /// Groups in the source, before deduplication.
    public var importedGroups: Int
    /// The `preImport` backup of the library as it was before merging.
    public var backup: BackupInfo?
    /// What deduplication merged away; `importedSnippets - mergedSnippets` is roughly what was new.
    public var dedupeReport: DedupeReport?
    public var warnings: [ImportWarning]
    public var groupedWarnings: [WarningGroup]?

    public init(
        importedSnippets: Int,
        importedGroups: Int,
        backup: BackupInfo? = nil,
        dedupeReport: DedupeReport? = nil,
        warnings: [ImportWarning] = [],
        groupedWarnings: [WarningGroup]? = nil
    ) {
        self.importedSnippets = importedSnippets
        self.importedGroups = importedGroups
        self.backup = backup
        self.dedupeReport = dedupeReport
        self.warnings = warnings
        self.groupedWarnings = groupedWarnings
    }
}

/// An Espanso configuration found on disk (`detect_espanso`).
public struct EspansoDetection: Codable, Hashable, Sendable {
    public var path: String
    public var snippetCount: Int
    public var groupCount: Int

    public init(path: String, snippetCount: Int, groupCount: Int) {
        self.path = path
        self.snippetCount = snippetCount
        self.groupCount = groupCount
    }
}
