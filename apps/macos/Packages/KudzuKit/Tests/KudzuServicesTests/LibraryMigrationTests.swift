import Foundation
import KudzuCore
import SwiftData
@testable import KudzuServices
import Testing

@Suite struct LibraryMigrationTests {
    func scratch() throws -> URL {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("kudzu-migration-\(UUID())")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }
    @Test func copyVerifySwitchRestartPreservesSourceAndLocalPayloads() async throws {
        let dir = try scratch(); defer { try? FileManager.default.removeItem(at: dir) }
        let snippet = Snippet(name: "عربي\0e\u{301}", triggers: [";abc"], content: "exact\0bytes")
        let source = Library(formatVersion: Library.currentFormatVersion, groups: [], snippets: [snippet], usage: [])
        let raw = Data(try KudzuJSON.encodeLibrary(source).utf8)
        try raw.write(to: dir.appendingPathComponent("library.json"))
        let blob = Data([0, 255, 2, 3])
        try FileManager.default.createDirectory(at: dir.appendingPathComponent("clipboard/items"), withIntermediateDirectories: true)
        try blob.write(to: dir.appendingPathComponent("clipboard/items/payload"))
        let migrated = try await LibraryMigration.open(directory: dir, defaultsDomain: nil)
        #expect(migrated.repository.snapshot == (try KudzuJSON.decodeLibrary(String(decoding: raw, as: UTF8.self))))
        try await migrated.repository.edit { $0.snippets[0].content = "newer" }
        let relaunched = try await LibraryMigration.open(directory: dir, defaultsDomain: nil)
        #expect(relaunched.repository.snapshot.snippets[0].content == "newer")
        #expect(try Data(contentsOf: dir.appendingPathComponent("library.json")) == raw)
        #expect(try Data(contentsOf: dir.appendingPathComponent("clipboard/items/payload")) == blob)
        let recovery = try FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil).first { $0.lastPathComponent.hasPrefix("migration-recovery-") }!
        #expect(try Data(contentsOf: recovery.appendingPathComponent("library.json")) == raw)
        #expect(try Data(contentsOf: recovery.appendingPathComponent("clipboard/items/payload")) == blob)
    }
    @Test(arguments: [LibraryMigration.Stage.copied, .imported, .verified, .activating])
    func interruptionRetriesWithoutDuplicates(stage: LibraryMigration.Stage) async throws {
        let dir = try scratch(); defer { try? FileManager.default.removeItem(at: dir) }
        let source = Library(formatVersion: Library.currentFormatVersion, groups: [], snippets: [Snippet(name: "one", triggers: [";one"], content: "one")], usage: [])
        let raw = Data(try KudzuJSON.encodeLibrary(source).utf8)
        try raw.write(to: dir.appendingPathComponent("library.json"))
        await #expect(throws: (any Error).self) {
            _ = try await LibraryMigration.open(directory: dir, defaultsDomain: nil) { if $0 == stage { throw ServiceError("interrupted") } }
        }
        #expect(!FileManager.default.fileExists(atPath: dir.appendingPathComponent("active-library.json").path))
        let reopened = try await LibraryMigration.open(directory: dir, defaultsDomain: nil)
        #expect(reopened.repository.snapshot == (try KudzuJSON.decodeLibrary(String(decoding: raw, as: UTF8.self))))
        #expect(try Data(contentsOf: dir.appendingPathComponent("library.json")) == raw)
    }
    @Test func unreadableSourceFailsThenRetries() async throws {
        let dir = try scratch(); defer { try? FileManager.default.removeItem(at: dir) }
        let url = dir.appendingPathComponent("library.json")
        try Data("broken".utf8).write(to: url)
        await #expect(throws: (any Error).self) { _ = try await LibraryMigration.open(directory: dir, defaultsDomain: nil) }
        #expect(try String(contentsOf: url, encoding: .utf8) == "broken")
        try Data(try KudzuJSON.encodeLibrary(.empty).utf8).write(to: url)
        let result = try await LibraryMigration.open(directory: dir, defaultsDomain: nil)
        #expect(result.repository.snapshot.snippets.isEmpty)
    }
    @Test func externalContextChangesSurviveUnrelatedEditAndReload() async throws {
        let dir = try scratch(); defer { try? FileManager.default.removeItem(at: dir) }
        let url = dir.appendingPathComponent("store")
        let repository = try await LibraryRepository.open(.local(url: url))
        let original = Snippet(name: "local", triggers: [";local"], content: "before")
        try await repository.edit { $0.snippets.append(original) }
        let second = try await LibraryRepository.open(.local(url: url))
        let remote = Snippet(name: "external", triggers: [";remote"], content: "imported")
        try await second.edit { library in library.snippets[0].content = "external edit"; library.snippets.append(remote) }
        try await repository.edit { library in library.groups.append(Group(name: "unrelated")) }
        #expect(repository.snapshot.snippets.count == 2)
        #expect(repository.snapshot.snippets[0].content == "external edit")
        #expect(try await second.reload().groups.count == 1)
    }
    @Test func unknownDestinationIsNeverReplaced() async throws {
        let dir = try scratch(); defer { try? FileManager.default.removeItem(at: dir) }
        let raw = Data("unrecognized store".utf8)
        try raw.write(to: dir.appendingPathComponent("Library.store"))
        await #expect(throws: (any Error).self) { _ = try await LibraryMigration.open(directory: dir, defaultsDomain: nil) }
        #expect(try Data(contentsOf: dir.appendingPathComponent("Library.store")) == raw)
        #expect(!FileManager.default.fileExists(atPath: dir.appendingPathComponent("active-library.json").path))
    }

    @Test func joinDoesNotSeedLegacySource() async throws {
        let dir = try scratch(); defer { try? FileManager.default.removeItem(at: dir) }
        let source = Library(formatVersion: Library.currentFormatVersion, groups: [], snippets: [Snippet(name: "preserved", triggers: [], content: "local")], usage: [])
        let raw = Data(try KudzuJSON.encodeLibrary(source).utf8)
        try raw.write(to: dir.appendingPathComponent("library.json"))
        let joined = try await LibraryMigration.open(directory: dir, defaultsDomain: nil, role: .join)
        #expect(joined.repository.snapshot == .empty)
        #expect(try Data(contentsOf: dir.appendingPathComponent("library.json")) == raw)
    }
}

@Suite struct RepositoryAgentIntegrationTests {
    struct Host: Agent.Host, Sendable {
        func access() throws -> Agent.Access { .all }
        func library() throws -> Library { .empty }
        func editLibrary(_ edit: (inout Library) throws -> [String: Any]) throws -> [String: Any] { throw Agent.Failure("outside executor") }
        func clipboard() throws -> [ClipEntry] { [] }
        func clipboardText(_ id: UUID) throws -> String? { nil }
        func renderEnv() -> Agent.RenderEnv { .init(now: Date(), locale: "en_US") }
    }
    @Test func wireMutationIsDurableBeforeReply() async throws {
        let dir = scratchStoreDir("agent-wire"); defer { try? FileManager.default.removeItem(at: dir) }
        let repository = try await LibraryRepository.open(.local(url: dir.appendingPathComponent("store")))
        let message = #"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"create_snippet","arguments":{"name":"MCP","content":"durable","triggers":[";mcp"]}}}"#
        let reply = try await repository.handleAgent(message: message, version: "0.7.0", host: Host())
        #expect(reply?.contains("isError\":true") == false)
        let reopened = try await LibraryRepository.open(.local(url: dir.appendingPathComponent("store")))
        #expect(reopened.snapshot.snippets.contains { $0.name == "MCP" && $0.content == "durable" })
    }
    @Test func shutdownQueueDrainsAcceptedWork() async throws {
        let queue = AsyncWorkQueue()
        let dir = scratchStoreDir("drain"); defer { try? FileManager.default.removeItem(at: dir) }
        let repository = try await LibraryRepository.open(.local(url: dir.appendingPathComponent("store")))
        for index in 0..<20 {
            #expect(queue.enqueue { try? await repository.edit { $0.groups.append(Group(name: "\(index)")) } })
        }
        await queue.drain()
        #expect(repository.snapshot.groups.map(\.name) == (0..<20).map(String.init))
        #expect(!queue.enqueue {})
    }
    @Test func localSaveDoesNotClaimNetworkSuccess() async throws {
        let status = AutomaticCloudStatus(enabled: true, available: false, publish: { _ in })
        status.recheck()
        #expect(status.currentStatus == .unavailable(reason: "This build is local-only; fresh iCloud provisioning is required."))
    }
}
