import Foundation
import SwiftData

/// Schema version 1, the first shipped CloudKit schema. Later versions may only add attributes or models.
public enum KudzuSchemaV1: VersionedSchema {
    public static let versionIdentifier = Schema.Version(1, 0, 0)

    public static var models: [any PersistentModel.Type] {
        syncedModels + localModels
    }

    /// Models in the "Synced" configuration.
    public static let syncedModels: [any PersistentModel.Type] = [
        SnippetModel.self, GroupModel.self, UsageModel.self, SyncedSettingsModel.self,
    ]

    /// Models in the "Local" configuration.
    public static let localModels: [any PersistentModel.Type] = [LocalSettingsModel.self]
}
