import Foundation
import KudzuCore
@testable import KudzuApp
@testable import KudzuServices
import Testing

@Suite struct SwiftDataBackendTests {
    @Test @MainActor func releaseBootstrapPreservesThePrereleaseDirectory() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("kudzu-legacy-bootstrap-\(UUID())")
        defer { try? FileManager.default.removeItem(at: root) }
        let legacy = root.appendingPathComponent(AppIdentity.legacyID)
        try FileManager.default.createDirectory(at: legacy.appendingPathComponent("nested"), withIntermediateDirectories: true)
        let source = Library(snippets: [Snippet(id: UUID(), name: "Legacy", content: "keep", createdAt: Date(timeIntervalSince1970: 100))])
        let bytes = Data(try KudzuJSON.encodeLibrary(source).utf8)
        try bytes.write(to: legacy.appendingPathComponent("library.json"))
        try Data("legacy-device".utf8).write(to: legacy.appendingPathComponent("device-id"))
        let payload = Data([0, 1, 255])
        try payload.write(to: legacy.appendingPathComponent("nested/local.bin"))
        let directory = root.appendingPathComponent(AppIdentity.productionID)
        let identity = AppIdentity(bundleID: AppIdentity.productionID, version: "0.7.0", productName: "Kudzu")
        let backend = try await LiveBackend.open(directory: directory, identity: identity)
        #expect(!backend.store.firstRun)
        #expect(backend.store.library == source)
        #expect(backend.store.deviceID == "legacy-device")
        #expect(try Data(contentsOf: directory.appendingPathComponent("nested/local.bin")) == payload)
        #expect(try Data(contentsOf: legacy.appendingPathComponent("library.json")) == bytes)
        await backend.shutDown()
    }

    @Test @MainActor func durableCRUDUndoImportExportAndBackups() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("kudzu-backend-\(UUID())")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let identity = AppIdentity(bundleID: AppIdentity.devID, version: "0.6.0", productName: "Kudzu Dev")
        let backend = try await LiveBackend.open(directory: dir, identity: identity)
        let group = try await backend.createGroup(name: "Test", parentId: nil)
        let snippet = try await backend.createSnippet(draft: SnippetDraft(name: "Native", triggers: [";native"], content: "body", groupID: group.id))
        #expect(backend.store.library.snippets.contains { $0.id == snippet.id })
        let backup = try await backend.backupNow()
        try await backend.deleteSnippets(ids: [snippet.id])
        #expect(!backend.store.library.snippets.contains { $0.id == snippet.id })
        try await backend.restoreSnippets([snippet])
        #expect(backend.store.library.snippets.contains { $0.id == snippet.id })
        let export = dir.appendingPathComponent("export.json")
        try await backend.exportJson(path: export.path)
        let exported = try KudzuJSON.decodeLibrary(String(contentsOf: export, encoding: .utf8))
        #expect(exported.snippets.contains { $0.id == snippet.id })
        _ = try await backend.importFile(path: export.path)
        #expect(try await backend.listBackups().contains { $0.reason == .preImport })
        try await backend.restoreBackup(path: backup.path)
        #expect(try await backend.listBackups().contains { $0.reason == .preRestore })
        let reopened = try await LibraryRepository.open(.local(url: dir.appendingPathComponent("Library.store")))
        #expect(reopened.snapshot == backend.store.library)
        await backend.shutDown()
    }
    @Test @MainActor func repositorySaveFailureReachesBackendCaller() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("kudzu-backend-failure-\(UUID())")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let identity = AppIdentity(bundleID: AppIdentity.devID, version: "0.6.0", productName: "Kudzu Dev")
        let backend = try await LiveBackend.open(directory: dir, identity: identity)
        let before = backend.store.library
        await backend.store.repository.testingSetFaultBeforeSave(true)
        await #expect(throws: (any Error).self) { _ = try await backend.createGroup(name: "Rejected", parentId: nil) }
        #expect(backend.store.library == before)
        await backend.store.repository.testingSetFaultBeforeSave(false)
        _ = try await backend.createGroup(name: "Retry", parentId: nil)
        #expect(backend.store.library.groups.contains { $0.name == "Retry" })
        await backend.shutDown()
    }
}
