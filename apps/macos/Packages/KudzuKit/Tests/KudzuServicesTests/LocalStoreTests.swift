import Foundation
import KudzuCore
import Testing
@testable import KudzuServices

@Suite struct LocalStoreTests {
    @Test func localSettingsFormMemoryAndDeviceIdentitySurviveReopen() async throws {
        let dir = scratchStoreDir("local-state")
        defer { try? FileManager.default.removeItem(at: dir) }
        let repository = try await openLocalRepository(at: dir)
        let store = try LocalStore(dir: dir, repository: repository, defaults: .devDefault, firstRun: true)
        store.withLock {
            $0.settings.defaultTriggerPrefix = ":"
            $0.formMemory.record(["Name": "Ada"], for: uuid("a1"))
        }
        store.saveSettings()
        store.saveFormMemory()
        store.flush()
        let reopened = try LocalStore(dir: dir, repository: repository, defaults: .devDefault, firstRun: false)
        #expect(reopened.settings.defaultTriggerPrefix == ":")
        #expect(reopened.formMemory.values[uuid("a1")] == ["Name": "Ada"])
        #expect(reopened.deviceID == store.deviceID)
        #expect(!FileManager.default.fileExists(atPath: dir.appendingPathComponent("library.json").path))
    }

    @Test func missingSettingKeysUseTheSelectedVariantDefaults() async throws {
        let dir = scratchStoreDir("local-defaults")
        defer { try? FileManager.default.removeItem(at: dir) }
        let repository = try await openLocalRepository(at: dir)
        try Data("{}".utf8).write(to: dir.appendingPathComponent("settings.json"))
        let dev = try LocalStore(dir: dir, repository: repository, defaults: .devDefault, firstRun: false)
        let release = try LocalStore(dir: dir, repository: repository, defaults: .default, firstRun: false)
        #expect(dev.settings == .devDefault)
        #expect(release.settings == .default)
        #expect(dev.deviceID == release.deviceID)
    }

    @Test func aRestartRequirementSurvivesStatusRechecks() {
        let status = AutomaticCloudStatus(enabled: true, available: false, publish: { _ in })
        status.configurationChanged()
        status.recheck()
        #expect(status.currentStatus == .error(message: "Restart Kudzu to apply the iCloud setting."))
    }
}
