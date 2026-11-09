import Foundation
import SwiftData

/// Where the SwiftData library store lives and whether it mirrors to CloudKit.
///
/// There is deliberately no `.automatic` case. The shipping app's custom sync writes its own
/// record types into `iCloud.sh.saeed.kudzu`; SwiftData's automatic mirroring would expect
/// `CD_*` record types there. Mirroring therefore needs a fresh container whose identifier is
/// named explicitly by the caller, and this type refuses the legacy identifier outright.
public enum LibraryStoreConfiguration: Sendable, Equatable {
    /// An on-disk store with CloudKit mirroring off. Tests use this with a scratch directory.
    case local(url: URL)
    /// An on-disk store mirrored to the private CloudKit database of the named container.
    /// Requires the iCloud entitlement for that container on the running process.
    case cloudKit(containerIdentifier: String, url: URL)

    /// The container the current `CloudSync` uses with its custom schema. Never mirrored here.
    public static let destinationContainerIdentifier = "iCloud.sh.saeed.kudzu.library"
    public static let legacyContainerIdentifier = "iCloud.sh.saeed.kudzu"

    /// The store file.
    public var url: URL {
        switch self {
        case let .local(url), let .cloudKit(_, url): url
        }
    }

    /// The CloudKit container this configuration mirrors to, or `nil` for a local-only store.
    public var cloudKitContainerIdentifier: String? {
        switch self {
        case .local: nil
        case let .cloudKit(identifier, _): identifier
        }
    }

    /// Validates the configuration without opening anything.
    public func validate() throws {
        guard case let .cloudKit(identifier, _) = self else { return }
        if identifier.isEmpty {
            throw LibraryRepositoryError.emptyContainerIdentifier
        }
        if identifier == Self.legacyContainerIdentifier {
            throw LibraryRepositoryError.legacyContainerRefused(identifier)
        }
    }

    /// The SwiftData configuration. `allowsSave` is always true; the store file is `url`.
    func modelConfiguration(schema: Schema) throws -> ModelConfiguration {
        try validate()
        let database: ModelConfiguration.CloudKitDatabase
        switch self {
        case .local: database = .none
        case let .cloudKit(identifier, _): database = .private(identifier)
        }
        return ModelConfiguration("KudzuLibrary", schema: schema, url: url, allowsSave: true, cloudKitDatabase: database)
    }
}

/// Failures of ``LibraryRepository`` that are not plain SwiftData/Core Data errors.
public enum LibraryRepositoryError: Error, Equatable, Sendable, LocalizedError {
    /// Two or more rows carry the same identity. Nothing is chosen silently; a resolution policy
    /// belongs to the live-sync step, not to local persistence.
    case duplicateIdentity(entity: String, keys: [String])
    /// A stored payload or raw enum value does not decode into the Core type. The row is named;
    /// no default is substituted.
    case unreadableField(entity: String, id: String, field: String, reason: String)
    /// `.cloudKit` named the legacy custom-schema container.
    case legacyContainerRefused(String)
    /// `.cloudKit` named no container.
    case emptyContainerIdentifier
    /// Test-only: a fault injected after rows were changed and before the transaction saved.
    case unsupportedFormatVersion(Int)
    case deletedDuringEdit(entity: String, id: String)
    case injectedFault

    public var errorDescription: String? {
        switch self {
        case let .duplicateIdentity(entity, keys):
            "\(entity) rows share an identity: \(keys.joined(separator: ", "))"
        case let .unreadableField(entity, id, field, reason):
            "\(entity) \(id): \(field) could not be read (\(reason))"
        case let .legacyContainerRefused(identifier):
            "\(identifier) holds the previous sync schema and cannot be mirrored by SwiftData"
        case .emptyContainerIdentifier:
            "A CloudKit container identifier is required"
        case let .unsupportedFormatVersion(version):
            "Unsupported library format version \(version)"
        case let .deletedDuringEdit(entity, id):
            "The \(entity) \(id) was deleted by another writer. Your change was not saved; reload before retrying."
        case .injectedFault:
            "Injected fault before save"
        }
    }
}
