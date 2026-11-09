import CoreData
import Darwin
import Foundation
import KudzuCore
import SwiftData
import Testing
@testable import KudzuServices

@Suite(.serialized) struct LibraryIntegrationRegressionTests {
    @Test func aPostSaveRefreshFailureDoesNotRejectCommittedWork() async throws {
        let dir = scratchStoreDir("post-save-result")
        defer { try? FileManager.default.removeItem(at: dir) }
        let repository = try await openLocalRepository(at: dir)
        let original = Snippet(id: uuid("91"), content: "original", createdAt: t0)
        try await repository.importLibrary(Library(snippets: [original]))
        let container = repository.modelContainer
        let storeURL = dir.appendingPathComponent("Library.store").resolvingSymlinksInPath()
        let once = SaveObservationOnce()
        let observer = NotificationCenter.default.addObserver(forName: .NSManagedObjectContextDidSave, object: nil, queue: nil) { note in
            guard let context = note.object as? NSManagedObjectContext,
                  context.persistentStoreCoordinator?.persistentStores.contains(where: { $0.url?.resolvingSymlinksInPath() == storeURL }) == true,
                  let inserted = note.userInfo?[NSInsertedObjectsKey] as? Set<NSManagedObject>,
                  inserted.contains(where: { $0.entity.name == "PersistedGroup" }) else { return }
            guard once.claim() else { return }
            do {
                let context = ModelContext(container)
                context.autosaveEnabled = false
                let duplicate = PersistedSnippet()
                try LibraryRowCodec.apply(original, listOrder: 0, to: duplicate)
                context.insert(duplicate)
                try context.save()
            } catch { Issue.record("External save failed: \(error)") }
        }
        defer { NotificationCenter.default.removeObserver(observer) }
        let created = try await repository.edit {
            try LibraryEditing.createGroup(name: "Committed", parentID: nil, in: &$0, id: uuid("92"))
        }
        #expect(created.id == uuid("92"))
        #expect(repository.snapshot.groups.map(\.id) == [created.id])
        let context = ModelContext(container)
        #expect(try context.fetchCount(FetchDescriptor<PersistedGroup>()) == 1)
        #expect(try context.fetchCount(FetchDescriptor<PersistedSnippet>()) == 2)
        #expect(repository.lastRefreshFailure?.contains("Duplicate library identities") == true)
        // The duplicate is reported on refresh; the completed command must not be retried.
        await #expect(throws: (any Error).self) { _ = try await repository.reload() }
    }

    @Test func anUnrelatedEditDoesNotRecreateExternallyDeletedRows() async throws {
        let dir = scratchStoreDir("external-deletion")
        defer { try? FileManager.default.removeItem(at: dir) }
        let repository = try await openLocalRepository(at: dir)
        let original = Snippet(id: uuid("93"), content: "original", createdAt: t0)
        try await repository.importLibrary(Library(groups: [Group(id: uuid("94"), name: "old")], snippets: [original], usage: [UsageRecord(snippetID: original.id, deviceID: "device", count: 2)]))
        let container = repository.modelContainer
        try await repository.edit { library in
            let context = ModelContext(container)
            for row in try context.fetch(FetchDescriptor<PersistedSnippet>()) { context.delete(row) }
            for row in try context.fetch(FetchDescriptor<PersistedGroup>()) { context.delete(row) }
            for row in try context.fetch(FetchDescriptor<PersistedUsage>()) { context.delete(row) }
            try context.save()
            library.groups.append(Group(id: uuid("95"), name: "new"))
        }
        #expect(repository.snapshot.snippets.isEmpty)
        #expect(repository.snapshot.usage.isEmpty)
        #expect(repository.snapshot.groups.map(\.id) == [uuid("95")])
        let reopened = try await openLocalRepository(at: dir)
        #expect(reopened.snapshot == repository.snapshot)
    }

    enum DeletedKind: CaseIterable { case snippet, group, usage }
    @Test(arguments: DeletedKind.allCases)
    func editingAnExternallyDeletedRowRejectsBeforeSaving(kind: DeletedKind) async throws {
        let dir = scratchStoreDir("deleted-edit")
        defer { try? FileManager.default.removeItem(at: dir) }
        let repository = try await openLocalRepository(at: dir)
        try await repository.importLibrary(Library(groups: [Group(id: uuid("96"), name: "group")], snippets: [Snippet(id: uuid("97"), content: "snippet", createdAt: t0)], usage: [UsageRecord(snippetID: uuid("97"), deviceID: "device", count: 1)]))
        let container = repository.modelContainer
        await #expect(throws: (any Error).self) {
            try await repository.edit { library in
                let context = ModelContext(container)
                switch kind {
                case .snippet:
                    for row in try context.fetch(FetchDescriptor<PersistedSnippet>()) { context.delete(row) }
                    library.snippets[0].content = "edited"
                case .group:
                    for row in try context.fetch(FetchDescriptor<PersistedGroup>()) { context.delete(row) }
                    library.groups[0].name = "edited"
                case .usage:
                    for row in try context.fetch(FetchDescriptor<PersistedUsage>()) { context.delete(row) }
                    library.usage[0].count += 1
                }
                try context.save()
                library.groups.append(Group(id: uuid("98"), name: "must roll back"))
            }
        }
        let reopened = try await openLocalRepository(at: dir)
        #expect(!reopened.snapshot.groups.contains { $0.id == uuid("98") })
        switch kind {
        case .snippet: #expect(reopened.snapshot.snippets.isEmpty)
        case .group: #expect(reopened.snapshot.groups.isEmpty)
        case .usage: #expect(reopened.snapshot.usage.isEmpty)
        }
    }

    @Test func aStaleAgentSocketDoesNotPreventMigration() async throws {
        let dir = URL(fileURLWithPath: "/private/tmp/kz-socket-\(UUID().uuidString.prefix(8))")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try Data(try KudzuJSON.encodeLibrary(.empty).utf8).write(to: dir.appendingPathComponent("library.json"))
        let socket = dir.appendingPathComponent("agent.sock")
        let fd = try UnixSocket.listen(socket.path)
        close(fd)
        let migrated = try await LibraryMigration.open(directory: dir, defaultsDomain: nil)
        #expect(migrated.repository.snapshot == .empty)
        #expect(FileManager.default.fileExists(atPath: socket.path))
        #expect(FileManager.default.fileExists(atPath: dir.appendingPathComponent("active-library.json").path))
    }

    @Test func linkedRecoveryPayloadIsRefusedWithoutChangingItsTarget() async throws {
        let dir = scratchStoreDir("linked-recovery")
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = Data(try KudzuJSON.encodeLibrary(.empty).utf8)
        try source.write(to: dir.appendingPathComponent("library.json"))
        let target = dir.deletingLastPathComponent().appendingPathComponent("kudzu-link-target-\(UUID())")
        defer { try? FileManager.default.removeItem(at: target) }
        let bytes = Data("target stays unchanged".utf8)
        try bytes.write(to: target)
        try FileManager.default.setAttributes([.posixPermissions: 0o644], ofItemAtPath: target.path)
        try FileManager.default.createSymbolicLink(at: dir.appendingPathComponent("linked-file"), withDestinationURL: target)
        await #expect(throws: (any Error).self) { _ = try await LibraryMigration.open(directory: dir, defaultsDomain: nil) }
        #expect(try Data(contentsOf: target) == bytes)
        #expect((try FileManager.default.attributesOfItem(atPath: target.path)[.posixPermissions] as? NSNumber)?.intValue == 0o644)
        #expect(!FileManager.default.fileExists(atPath: dir.appendingPathComponent("active-library.json").path))
        #expect(try Data(contentsOf: dir.appendingPathComponent("library.json")) == source)
    }

    @Test(arguments: [Data(), Data("not an initialized store".utf8)])
    func aDamagedActiveStoreIsNeverInitialized(bytes: Data) async throws {
        let dir = scratchStoreDir("damaged-active")
        defer { try? FileManager.default.removeItem(at: dir) }
        let url = dir.appendingPathComponent("Library.store")
        try bytes.write(to: url)
        try Data(#"{"version":1,"role":"primary","recovery":"kept","store":"Library.store"}"#.utf8).write(to: dir.appendingPathComponent("active-library.json"))
        await #expect(throws: (any Error).self) { _ = try await LibraryMigration.open(directory: dir, defaultsDomain: nil) }
        #expect(try Data(contentsOf: url) == bytes)
    }

    @Test func anInitializedEmptyActiveLibraryRemainsValid() async throws {
        let dir = scratchStoreDir("initialized-empty")
        defer { try? FileManager.default.removeItem(at: dir) }
        try Data(try KudzuJSON.encodeLibrary(.empty).utf8).write(to: dir.appendingPathComponent("library.json"))
        _ = try await LibraryMigration.open(directory: dir, defaultsDomain: nil)
        let reopened = try await LibraryMigration.open(directory: dir, defaultsDomain: nil)
        #expect(reopened.repository.snapshot == .empty)
        #expect(!reopened.firstRun)
    }

    @Test func aMissingActiveStoreIsNotRecreated() async throws {
        let dir = scratchStoreDir("missing-active")
        defer { try? FileManager.default.removeItem(at: dir) }
        try Data(#"{"version":1,"role":"primary","recovery":"kept","store":"Library.store"}"#.utf8).write(to: dir.appendingPathComponent("active-library.json"))
        await #expect(throws: (any Error).self) { _ = try await LibraryMigration.open(directory: dir, defaultsDomain: nil) }
        #expect(!FileManager.default.fileExists(atPath: dir.appendingPathComponent("Library.store").path))
    }

    @Test func aChangedSourceCannotActivateAnUnmatchedRecoveryCopy() async throws {
        let dir = scratchStoreDir("changed-source")
        defer { try? FileManager.default.removeItem(at: dir) }
        let sourceURL = dir.appendingPathComponent("library.json")
        let original = Data(try KudzuJSON.encodeLibrary(Library(snippets: [Snippet(id: uuid("99"), content: "original", createdAt: t0)])).utf8)
        let changed = Data(try KudzuJSON.encodeLibrary(.empty).utf8)
        try original.write(to: sourceURL)
        await #expect(throws: (any Error).self) {
            _ = try await LibraryMigration.open(directory: dir, defaultsDomain: nil) { stage in
                if case .copied = stage { try changed.write(to: sourceURL) }
            }
        }
        #expect(!FileManager.default.fileExists(atPath: dir.appendingPathComponent("active-library.json").path))
        #expect(try Data(contentsOf: sourceURL) == changed)
        let retried = try await LibraryMigration.open(directory: dir, defaultsDomain: nil)
        #expect(retried.repository.snapshot == .empty)
    }
}

private final class SaveObservationOnce: @unchecked Sendable {
    private let lock = NSLock()
    private var used = false
    func claim() -> Bool { lock.withLock { guard !used else { return false }; used = true; return true } }
}
