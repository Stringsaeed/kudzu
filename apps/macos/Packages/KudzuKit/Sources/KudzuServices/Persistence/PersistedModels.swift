import Foundation
import SwiftData

// The three independently synced record types. Rules for automatic CloudKit mirroring:
// every attribute has a default or is optional, there is no `#Unique`, and there are no
// relationships (membership is a plain `UUID?`, as on the Core value types), so no row depends
// on another row having arrived. `id` is not unique-constrained; the repository reports
// duplicates instead of choosing one.
//
// Row identity: snippets and groups by `id`; usage by `(snippetID, deviceID)`, the pair
// `DataStore.recordUsage` already looks up. `listOrder` keeps the observable array order of
// `Library.snippets` / `.groups` / `.usage`.
//
// Sentinel defaults are deliberately "obviously unset" values: a row that still carries them was
// created without the codec and is a bug, not data.

/// Schema version 1. Later versions may only add attributes or models (CloudKit Production is add-only).
enum KudzuLibrarySchemaV1: VersionedSchema {
    static let versionIdentifier = Schema.Version(1, 0, 0)

    static var models: [any PersistentModel.Type] {
        [PersistedSnippet.self, PersistedGroup.self, PersistedUsage.self]
    }

    static var schema: Schema { Schema(versionedSchema: KudzuLibrarySchemaV1.self) }
}

enum PersistedDefaults {
    static let nullUUID = UUID(uuid: UUID_NULL)
}

@Model
final class PersistedSnippet {
    var id: UUID = PersistedDefaults.nullUUID
    var nameUTF8: Data = Data()
    /// `Snippet.triggers` as the JSON array `KudzuJSON` writes.
    var triggersJSON: Data = Data()
    var contentUTF8: Data = Data()
    /// `Snippet.formatting` as `KudzuJSON` writes it (source, spans with links and image data
    /// URLs); `nil` when the snippet has none. External storage keeps large image payloads out
    /// of the row and lets CloudKit mirror them as assets.
    @Attribute(.externalStorage) var formattingJSON: Data?
    var groupID: UUID?
    var fireModeRaw: String = ""
    var caseModeRaw: String = ""
    var insertionRaw: String?
    /// `Snippet.rule` as `KudzuJSON` writes it; `nil` inherits.
    var ruleJSON: Data?
    var isFavorite: Bool = false
    var isEnabled: Bool = true
    var createdAt: Date = Date.distantPast
    var updatedAt: Date = Date.distantPast
    /// Position in `Library.snippets`.
    var listOrder: Int = 0

    init() {}
}

@Model
final class PersistedGroup {
    var id: UUID = PersistedDefaults.nullUUID
    var nameUTF8: Data = Data()
    var parentID: UUID?
    var sortIndex: Int = 0
    var isEnabled: Bool = true
    var insertionRaw: String?
    var ruleJSON: Data?
    /// Position in `Library.groups`.
    var listOrder: Int = 0

    init() {}
}

@Model
final class PersistedUsage {
    var snippetID: UUID = PersistedDefaults.nullUUID
    var deviceIDUTF8: Data = Data()
    var count: Int = 0
    var charsSaved: Int = 0
    var lastUsedAt: Date?
    /// Position in `Library.usage`.
    var listOrder: Int = 0

    init() {}
}
