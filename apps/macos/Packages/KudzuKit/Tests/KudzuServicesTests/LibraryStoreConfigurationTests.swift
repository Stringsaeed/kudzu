import Foundation
import KudzuCore
import SwiftData
import Testing
@testable import KudzuServices

// SDK-level checks of the schema and the CloudKit configuration gate. None of this talks to
// CloudKit: that needs a signed process with the entitlement for a fresh container.

@Suite("SwiftData schema and store configuration")
struct LibraryStoreConfigurationTests {
    @Test func schemaMeetsAutomaticCloudKitRules() throws {
        let schema = KudzuLibrarySchemaV1.schema
        #expect(Set(schema.entities.map(\.name)) == ["PersistedSnippet", "PersistedGroup", "PersistedUsage"])
        var report: [String] = []
        for entity in schema.entities.sorted(by: { $0.name < $1.name }) {
            #expect(entity.relationships.isEmpty, "\(entity.name) must not carry relationships")
            #expect(entity.uniquenessConstraints.isEmpty, "\(entity.name) must not carry uniqueness constraints")
            for attribute in entity.attributes.sorted(by: { $0.name < $1.name }) {
                #expect(!attribute.isUnique, "\(entity.name).\(attribute.name) is unique")
                #expect(attribute.isOptional || attribute.defaultValue != nil, "\(entity.name).\(attribute.name) has neither a default nor is optional")
                report.append("\(entity.name).\(attribute.name): optional=\(attribute.isOptional) default=\(attribute.defaultValue.map { String(describing: $0) } ?? "nil") unique=\(attribute.isUnique)")
            }
        }
        // 15 + 8 + 6 persisted attributes.
        #expect(report.count == 29, "\(report.joined(separator: "\n"))")
        let snippet = try #require(schema.entities.first { $0.name == "PersistedSnippet" })
        let formatting = try #require(snippet.attributes.first { $0.name == "formattingJSON" })
        #expect(formatting.options.contains(.externalStorage))
        #expect(formatting.isOptional)
    }

    @Test func cloudKitConfigurationRequiresAnExplicitFreshContainer() async throws {
        let dir = scratchStoreDir("config")
        defer { try? FileManager.default.removeItem(at: dir) }
        let url = dir.appendingPathComponent("Library.store")

        // The legacy container is refused before any file is created.
        await #expect(throws: LibraryRepositoryError.legacyContainerRefused("iCloud.sh.saeed.kudzu")) {
            _ = try await LibraryRepository.open(.cloudKit(containerIdentifier: LibraryStoreConfiguration.legacyContainerIdentifier, url: url))
        }
        await #expect(throws: LibraryRepositoryError.emptyContainerIdentifier) {
            _ = try await LibraryRepository.open(.cloudKit(containerIdentifier: "", url: url))
        }
        #expect(!FileManager.default.fileExists(atPath: url.path))

        // A fresh identifier validates; it is not opened here because this process has no
        // iCloud entitlement (see docs/SWIFTDATA_PROTOTYPE.md, "Live CloudKit route").
        let fresh = LibraryStoreConfiguration.cloudKit(containerIdentifier: "iCloud.sh.saeed.kudzu.library", url: url)
        try fresh.validate()
        #expect(fresh.cloudKitContainerIdentifier == "iCloud.sh.saeed.kudzu.library")
        #expect(LibraryStoreConfiguration.local(url: url).cloudKitContainerIdentifier == nil)

        // The local configuration builds and points at the file. (`CloudKitDatabase` is an opaque
        // struct without `Equatable`; the `.none` choice is asserted by reading the source and by
        // every disk test running without an iCloud entitlement.)
        let config = try LibraryStoreConfiguration.local(url: url).modelConfiguration(schema: KudzuLibrarySchemaV1.schema)
        #expect(config.url == url)
        #expect(config.allowsSave)
    }
}
