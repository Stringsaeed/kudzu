import Foundation

/// A group of import warnings that share the same message.
public struct WarningGroup: Codable, Hashable, Sendable, Identifiable {
    public var id: String { message }
    public var message: String
    public var warnings: [ImportWarning]
    public var count: Int { warnings.count }

    public init(message: String, warnings: [ImportWarning]) {
        self.message = message
        self.warnings = warnings
    }
}

/// A preview of an Espanso configuration directory before merging it into the store.
public struct EspansoDirectoryPreview: Hashable, Sendable, Identifiable {
    public var id: URL { url }
    public var url: URL
    public var result: EspansoImportResult
    public var groupedWarnings: [WarningGroup]

    public var snippetCount: Int { result.library.snippets.count }
    public var groupCount: Int { result.library.groups.count }

    public init(url: URL, result: EspansoImportResult) {
        self.url = url
        self.result = result
        self.groupedWarnings = ImportCoordinator.groupWarnings(result.warnings)
    }
}

/// Summary of a completed library import (Espanso or Kudzu JSON), including backup and dedupe details.
public struct ImportSummary: Hashable, Sendable {
    /// Number of snippets decoded from the source before deduplication.
    public var importedSnippets: Int
    /// Number of groups decoded from the source before deduplication.
    public var importedGroups: Int
    /// Location of the `.preImport` safety backup written before merging.
    public var backupURL: URL
    /// Deduplication report from `KudzuStore.merge(_:)`.
    public var dedupeReport: DedupeReport
    /// Raw warnings emitted during import.
    public var warnings: [ImportWarning]
    /// Warnings grouped by message for UI display.
    public var groupedWarnings: [WarningGroup]

    public init(
        importedSnippets: Int,
        importedGroups: Int,
        backupURL: URL,
        dedupeReport: DedupeReport,
        warnings: [ImportWarning] = []
    ) {
        self.importedSnippets = importedSnippets
        self.importedGroups = importedGroups
        self.backupURL = backupURL
        self.dedupeReport = dedupeReport
        self.warnings = warnings
        self.groupedWarnings = ImportCoordinator.groupWarnings(warnings)
    }
}

/// Coordinates library imports, exports, safety backups, warning summaries and starter snippets.
public enum ImportCoordinator {
    /// Starter trigger used in onboarding step 3.
    public static let sampleTrigger = ";kudzu"
    /// Starter expansion text used in onboarding step 3.
    public static let sampleExpansion = "Kudzu is working! 🌱"

    /// Groups warnings by their message, ordered by occurrence count (descending) then message.
    public static func groupWarnings(_ warnings: [ImportWarning]) -> [WarningGroup] {
        var buckets: [String: [ImportWarning]] = [:]
        for warning in warnings {
            buckets[warning.message, default: []].append(warning)
        }
        return buckets
            .map { WarningGroup(message: $0.key, warnings: $0.value) }
            .sorted { lhs, rhs in
                if lhs.count != rhs.count { return lhs.count > rhs.count }
                return lhs.message < rhs.message
            }
    }

    /// Detects existing default Espanso configuration directories and previews their contents.
    public static func detectEspansoDirectories(
        homeDirectory: URL = FileManager.default.homeDirectoryForCurrentUser,
        fileManager: FileManager = .default
    ) -> [EspansoDirectoryPreview] {
        var previews: [EspansoDirectoryPreview] = []
        for candidate in EspansoImporter.defaultConfigDirectories(homeDirectory: homeDirectory) {
            var isDir: ObjCBool = false
            guard fileManager.fileExists(atPath: candidate.path, isDirectory: &isDir), isDir.boolValue else {
                continue
            }
            if let preview = try? previewEspansoDirectory(candidate, fileManager: fileManager) {
                previews.append(preview)
            }
        }
        return previews
    }

    /// Runs the Espanso importer on `url` without modifying the store, returning a preview.
    public static func previewEspansoDirectory(
        _ url: URL,
        fileManager: FileManager = .default
    ) throws -> EspansoDirectoryPreview {
        let result = try EspansoImporter.importDirectory(url, fileManager: fileManager)
        return EspansoDirectoryPreview(url: url, result: result)
    }

    /// Imports an Espanso directory into `store` after writing a `.preImport` backup.
    @MainActor
    public static func importEspanso(
        from url: URL,
        into store: KudzuStore,
        backupStore: BackupStore,
        fileManager: FileManager = .default,
        now: Date = .now
    ) throws -> ImportSummary {
        let result = try EspansoImporter.importDirectory(url, fileManager: fileManager)
        return try importEspanso(result: result, into: store, backupStore: backupStore, now: now)
    }

    /// Merges a pre-computed `EspansoImportResult` into `store` after writing a `.preImport` backup.
    @MainActor
    public static func importEspanso(
        result: EspansoImportResult,
        into store: KudzuStore,
        backupStore: BackupStore,
        now: Date = .now
    ) throws -> ImportSummary {
        let currentLibrary = try store.library()
        let backupURL = try backupStore.write(currentLibrary, reason: .preImport, now: now)
        let report = try store.merge(result.library)
        return ImportSummary(
            importedSnippets: result.library.snippets.count,
            importedGroups: result.library.groups.count,
            backupURL: backupURL,
            dedupeReport: report,
            warnings: result.warnings
        )
    }

    /// Decodes a Kudzu JSON library file, writes a `.preImport` backup, and merges it into `store`.
    @MainActor
    public static func importKudzuJSON(
        from fileURL: URL,
        into store: KudzuStore,
        backupStore: BackupStore,
        now: Date = .now
    ) throws -> ImportSummary {
        let data = try Data(contentsOf: fileURL)
        return try importKudzuJSON(data: data, into: store, backupStore: backupStore, now: now)
    }

    /// Decodes Kudzu JSON data, writes a `.preImport` backup, and merges it into `store`.
    @MainActor
    public static func importKudzuJSON(
        data: Data,
        into store: KudzuStore,
        backupStore: BackupStore,
        now: Date = .now
    ) throws -> ImportSummary {
        let incoming = try KudzuJSON.decode(data)
        let currentLibrary = try store.library()
        let backupURL = try backupStore.write(currentLibrary, reason: .preImport, now: now)
        let report = try store.merge(incoming)
        return ImportSummary(
            importedSnippets: incoming.snippets.count,
            importedGroups: incoming.groups.count,
            backupURL: backupURL,
            dedupeReport: report
        )
    }

    /// Writes a `.manual` safety backup of the current library, then replaces the store with `info`.
    @MainActor
    @discardableResult
    public static func restoreBackup(
        _ info: BackupInfo,
        into store: KudzuStore,
        backupStore: BackupStore,
        now: Date = .now
    ) throws -> URL {
        let restored = try backupStore.load(info)
        let current = try store.library()
        let safetyBackupURL = try backupStore.write(current, reason: .manual, now: now)
        try store.replaceAll(with: restored)
        return safetyBackupURL
    }

    /// Exports `library` as Espanso YAML files inside `directory` and returns the written file URLs.
    @discardableResult
    public static func exportEspanso(
        library: KudzuLibrary,
        to directory: URL,
        fileManager: FileManager = .default
    ) throws -> [URL] {
        let exported = EspansoExporter.export(library)
        try fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
        var writtenURLs: [URL] = []
        for relPath in exported.keys.sorted() {
            guard let yaml = exported[relPath] else { continue }
            let fileURL = directory.appendingPathComponent(relPath, isDirectory: false)
            let parentDir = fileURL.deletingLastPathComponent()
            try fileManager.createDirectory(at: parentDir, withIntermediateDirectories: true)
            try Data(yaml.utf8).write(to: fileURL, options: .atomic)
            writtenURLs.append(fileURL)
        }
        return writtenURLs
    }

    /// Creates the "Getting Started" group and starter snippets (`;kudzu`, `;date`, `;sig`) if `;kudzu` is absent.
    ///
    /// - Returns: `true` if starter snippets were created, or `false` if `;kudzu` already existed.
    @MainActor
    @discardableResult
    public static func ensureGettingStartedSnippets(
        in store: KudzuStore,
        now: Date = .now
    ) throws -> Bool {
        let current = try store.library()
        let hasKudzuTrigger = current.snippets.contains { snippet in
            snippet.triggers.contains { $0.caseInsensitiveCompare(sampleTrigger) == .orderedSame }
        }
        guard !hasKudzuTrigger else { return false }

        let groupID: UUID
        if let existingGroup = current.groups.first(where: {
            $0.parentID == nil && $0.name.caseInsensitiveCompare("Getting Started") == .orderedSame
        }) {
            groupID = existingGroup.id
        } else {
            let nextSortIndex = (current.groups.filter { $0.parentID == nil }.map(\.sortIndex).max() ?? -1) + 1
            let group = GroupRecord(name: "Getting Started", parentID: nil, sortIndex: nextSortIndex)
            try store.upsert(group)
            groupID = group.id
        }

        let starterSnippets: [SnippetRecord] = [
            SnippetRecord(
                name: "Kudzu Test",
                triggers: [sampleTrigger],
                content: sampleExpansion,
                groupID: groupID,
                createdAt: now,
                updatedAt: now
            ),
            SnippetRecord(
                name: "Today's Date",
                triggers: [";date"],
                content: #"{{date format="EEEE, d MMMM yyyy"}}"#,
                groupID: groupID,
                createdAt: now,
                updatedAt: now
            ),
            SnippetRecord(
                name: "Signature",
                triggers: [";sig"],
                content: "Best regards,\nYour Name",
                groupID: groupID,
                createdAt: now,
                updatedAt: now
            ),
        ]

        for snippet in starterSnippets {
            try store.upsert(snippet)
        }
        return true
    }
}
