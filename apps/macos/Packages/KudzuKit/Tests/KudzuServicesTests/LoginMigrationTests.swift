import Foundation
import Testing
@testable import KudzuServices

@Suite struct LoginMigrationTests {
    @Test func failedRegistrationKeepsTheLegacyAgentForRetry() throws {
        enum Failure: Error { case registration }
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let agent = directory.appendingPathComponent("Kudzu.plist")
        let original = Data("recognized legacy agent fixture".utf8)
        try original.write(to: agent)
        #expect(throws: Failure.registration) {
            try LoginItem.migrateLegacyAgent(at: agent) { throw Failure.registration }
        }
        #expect(try Data(contentsOf: agent) == original)
        var registered = false
        try LoginItem.migrateLegacyAgent(at: agent) { registered = true }
        #expect(registered)
        #expect(!FileManager.default.fileExists(atPath: agent.path))
    }
}
