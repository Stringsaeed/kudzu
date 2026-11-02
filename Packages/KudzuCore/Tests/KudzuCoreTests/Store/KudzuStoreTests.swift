import Foundation
import SwiftData
import Testing
@testable import KudzuCore

@MainActor
@Suite struct KudzuStoreTests {
    @Test func containerHasSyncedAndLocalConfigurations() throws {
        let container = try KudzuStore.makeContainer(syncEnabled: true, inMemory: true, storeDirectory: nil)
        let names = Set(container.configurations.map(\.name))
        #expect(names == [KudzuStoreConstants.syncedConfigurationName, KudzuStoreConstants.localConfigurationName])
        // In-memory containers never talk to CloudKit, whatever `syncEnabled` says.
        #expect(container.configurations.allSatisfy { $0.cloudKitContainerIdentifier == nil })
    }

    @Test func onDiskContainerUsesStoreDirectory() throws {
        let dir = FileManager.default.temporaryDirectory.appending(path: "kudzu-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let container = try KudzuStore.makeContainer(syncEnabled: false, inMemory: false, storeDirectory: dir)
        let urls = Set(container.configurations.map(\.url.lastPathComponent))
        #expect(urls == [KudzuStoreConstants.syncedStoreFileName, KudzuStoreConstants.localStoreFileName])
        #expect(container.configurations.allSatisfy { $0.url.deletingLastPathComponent().standardizedFileURL.path == dir.standardizedFileURL.path })

        let store = KudzuStore(container: container)
        try store.upsert(SnippetRecord(name: "s", triggers: [";s"], content: "S"))
        let deviceID = try store.deviceID()
        // Reopen from disk: data and the device ID persist, each in its own store.
        let reopened = KudzuStore(container: try KudzuStore.makeContainer(syncEnabled: false, inMemory: false, storeDirectory: dir))
        #expect(try reopened.library().snippets.count == 1)
        #expect(try reopened.deviceID() == deviceID)
    }

    @Test func upsertInsertsThenUpdates() throws {
        let store = try KudzuStore.inMemory()
        var snippet = SnippetRecord(name: "a", triggers: [";a"], content: "A")
        try store.upsert(snippet)
        snippet.content = "AA"
        try store.upsert(snippet)
        #expect(try store.library().snippets == [snippet])
    }

    @Test func upsertResolvesGroupAndUnknownGroupBecomesRoot() throws {
        let store = try KudzuStore.inMemory()
        let group = GroupRecord(name: "g")
        try store.upsert(group)
        try store.upsert(SnippetRecord(name: "in", triggers: [";i"], content: "I", groupID: group.id))
        try store.upsert(SnippetRecord(name: "lost", triggers: [";l"], content: "L", groupID: UUID()))
        let snippets = try store.library().snippets
        #expect(snippets.first { $0.name == "in" }?.groupID == group.id)
        #expect(snippets.first { $0.name == "lost" }?.groupID == nil)
    }

    @Test func upsertGroupRejectsCycles() throws {
        let store = try KudzuStore.inMemory()
        var a = GroupRecord(name: "a")
        let b = GroupRecord(name: "b", parentID: a.id)
        try store.upsert(a)
        try store.upsert(b)
        a.parentID = b.id
        #expect(throws: KudzuStoreError.groupCycle) { try store.upsert(a) }
        a.parentID = a.id
        #expect(throws: KudzuStoreError.groupCycle) { try store.upsert(a) }
    }

    @Test func deleteSnippetRemovesItsUsage() throws {
        let store = try KudzuStore.inMemory()
        let keep = SnippetRecord(name: "k", triggers: [";k"], content: "K")
        let drop = SnippetRecord(name: "d", triggers: [";d"], content: "D")
        try store.upsert(keep)
        try store.upsert(drop)
        try store.recordUsage(snippetID: keep.id, charsSaved: 1)
        try store.recordUsage(snippetID: drop.id, charsSaved: 1)
        try store.deleteSnippet(id: drop.id)
        let library = try store.library()
        #expect(library.snippets.map(\.id) == [keep.id])
        #expect(library.usage.map(\.snippetID) == [keep.id])
    }

    @Test func deleteGroupMovesChildrenAndSnippets() throws {
        let store = try KudzuStore.inMemory()
        let target = GroupRecord(name: "target")
        let doomed = GroupRecord(name: "doomed")
        let child = GroupRecord(name: "child", parentID: doomed.id)
        let snippet = SnippetRecord(name: "s", triggers: [";s"], content: "S", groupID: doomed.id)
        for g in [target, doomed, child] { try store.upsert(g) }
        try store.upsert(snippet)

        // Moving into the group's own subtree is rejected.
        #expect(throws: KudzuStoreError.groupCycle) { try store.deleteGroup(id: doomed.id, moveChildrenTo: child.id) }

        try store.deleteGroup(id: doomed.id, moveChildrenTo: target.id)
        let library = try store.library()
        #expect(Set(library.groups.map(\.id)) == [target.id, child.id])
        #expect(library.groups.first { $0.id == child.id }?.parentID == target.id)
        #expect(library.snippets.first?.groupID == target.id)

        try store.deleteGroup(id: target.id, moveChildrenTo: nil)
        let afterRootMove = try store.library()
        #expect(afterRootMove.groups.map(\.parentID) == [nil])
        #expect(afterRootMove.snippets.first?.groupID == nil)
    }

    @Test func recordUsageIncrementsOnlyThisDevicesRow() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "s", triggers: [";s"], content: "Hello")
        try store.upsert(snippet)
        // Another device's row synced in.
        try store.merge(KudzuLibrary(usage: [UsageRecord(snippetID: snippet.id, deviceID: "other-mac", count: 10, charsSaved: 100)]))

        let t1 = Date(timeIntervalSinceReferenceDate: 1000), t2 = Date(timeIntervalSinceReferenceDate: 2000)
        try store.recordUsage(snippetID: snippet.id, charsSaved: 3, at: t2)
        try store.recordUsage(snippetID: snippet.id, charsSaved: 4, at: t1)

        let me = try store.deviceID()
        let usage = try store.library().usage
        #expect(usage.count == 2)
        #expect(usage.first { $0.deviceID == me } == UsageRecord(snippetID: snippet.id, deviceID: me, count: 2, charsSaved: 7, lastUsedAt: t2))
        #expect(usage.first { $0.deviceID == "other-mac" }?.count == 10)
    }

    @Test func deviceIDIsGeneratedOnceAndPersisted() throws {
        let container = try KudzuStore.makeContainer(syncEnabled: false, inMemory: true, storeDirectory: nil)
        let id = try KudzuStore(container: container).deviceID()
        #expect(!id.isEmpty)
        #expect(try KudzuStore(container: container).deviceID() == id)
        #expect(try KudzuStore.inMemory().deviceID() != id)
    }

    @Test func replaceAllRestoresExactly() throws {
        let store = try KudzuStore.inMemory()
        try store.upsert(SnippetRecord(name: "old", triggers: [";o"], content: "O"))
        try store.upsert(GroupRecord(name: "old group"))

        let parent = GroupRecord(name: "p", sortIndex: 1)
        let child = GroupRecord(name: "c", parentID: parent.id)
        let snippet = SnippetRecord(name: "s", triggers: [";s"], content: "S", groupID: child.id,
                                    createdAt: Date(timeIntervalSinceReferenceDate: 5), updatedAt: Date(timeIntervalSinceReferenceDate: 6))
        let usage = UsageRecord(snippetID: snippet.id, deviceID: "x", count: 3, charsSaved: 9)
        // Children listed before parents still resolve.
        let backup = KudzuLibrary(groups: [child, parent], snippets: [snippet], usage: [usage])
        try store.replaceAll(with: backup)

        let restored = try store.library()
        #expect(Set(restored.groups) == Set(backup.groups))
        #expect(restored.snippets == backup.snippets)
        #expect(restored.usage == backup.usage)
    }

    @Test func mergeAddsAndDedupes() throws {
        let store = try KudzuStore.inMemory()
        let existing = SnippetRecord(name: "sig", triggers: [";sig"], content: "Best", createdAt: Date(timeIntervalSinceReferenceDate: 1))
        try store.upsert(existing)
        try store.upsert(GroupRecord(name: "Work"))

        let importedGroup = GroupRecord(name: "work")
        let imported = KudzuLibrary(groups: [importedGroup], snippets: [
            SnippetRecord(name: "sig again", triggers: [";SIG"], content: "Best", groupID: importedGroup.id, isFavorite: true, createdAt: Date(timeIntervalSinceReferenceDate: 2)),
            SnippetRecord(name: "new", triggers: [";new"], content: "N", groupID: importedGroup.id),
            SnippetRecord(name: "clash", triggers: [";sig"], content: "Different"),
        ])
        let report = try store.merge(imported)
        #expect(report.mergedSnippets == 1)
        #expect(report.mergedGroups == 1)
        #expect(report.conflicts.map(\.trigger) == [";sig"])
        #expect(store.conflicts == report.conflicts)

        let library = try store.library()
        #expect(library.groups.count == 1)
        #expect(library.snippets.count == 3)
        let sig = try #require(library.snippets.first { $0.id == existing.id })
        #expect(sig.isFavorite)
        #expect(library.snippets.first { $0.name == "new" }?.groupID == library.groups[0].id)

        // Importing the same file again changes nothing.
        let again = try store.merge(imported)
        #expect(again.mergedSnippets == 1)
        #expect(try store.library() == library)
        #expect(try store.dedupe().isEmpty)
    }

    @Test func mergeOfOwnBackupDoesNotDoubleCountUsage() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "s", triggers: [";s"], content: "S")
        try store.upsert(snippet)
        try store.recordUsage(snippetID: snippet.id, charsSaved: 5)
        let backup = try store.library()
        try store.recordUsage(snippetID: snippet.id, charsSaved: 5)

        try store.merge(backup)
        #expect(try store.library().usage.map(\.count) == [2])
    }

    @Test func mergeKeepsNewerVersionOfSameSnippetID() throws {
        let store = try KudzuStore.inMemory()
        var snippet = SnippetRecord(name: "s", triggers: [";s"], content: "v2", updatedAt: Date(timeIntervalSinceReferenceDate: 20))
        try store.upsert(snippet)
        snippet.content = "v1"
        snippet.updatedAt = Date(timeIntervalSinceReferenceDate: 10)
        try store.merge(KudzuLibrary(snippets: [snippet]))
        #expect(try store.library().snippets.first?.content == "v2")
        snippet.content = "v3"
        snippet.updatedAt = Date(timeIntervalSinceReferenceDate: 30)
        try store.merge(KudzuLibrary(snippets: [snippet]))
        #expect(try store.library().snippets.first?.content == "v3")
    }

    /// Simulates what sync delivers when two Macs imported the same config:
    /// both copies land in one store, then the remote-change dedupe pass runs.
    @Test func dedupeAppliesPlanAndRecordsAliasesForLateUsage() throws {
        let store = try KudzuStore.inMemory()
        let a = SnippetRecord(name: "a", triggers: [";x"], content: "X", createdAt: Date(timeIntervalSinceReferenceDate: 1))
        let b = SnippetRecord(name: "b", triggers: [";x"], content: "X", createdAt: Date(timeIntervalSinceReferenceDate: 2))
        try store.replaceAll(with: KudzuLibrary(snippets: [a, b], usage: [
            UsageRecord(snippetID: a.id, deviceID: "A", count: 1, charsSaved: 1),
            UsageRecord(snippetID: b.id, deviceID: "B", count: 2, charsSaved: 2),
        ]))
        let report = try store.dedupe()
        #expect(report.mergedSnippets == 1)
        #expect(try store.library().snippets.map(\.id) == [a.id])
        #expect(Set(try store.library().usage.map(\.deviceID)) == ["A", "B"])
        #expect(try store.library().usage.allSatisfy { $0.snippetID == a.id })

        // Device B used `b` before learning it was merged; its row syncs in late.
        try store.merge(KudzuLibrary(usage: [UsageRecord(snippetID: b.id, deviceID: "B2", count: 7, charsSaved: 7)]))
        let usage = try store.library().usage
        #expect(usage.first { $0.deviceID == "B2" }?.snippetID == a.id)
        #expect(try store.dedupe().isEmpty)
    }

    @Test func duplicateSyncedSettingsCollapseToMostRecentlyEdited() throws {
        let store = try KudzuStore.inMemory()
        let edited = SyncedSettingsModel()
        edited.typingSpeedWPM = 90
        edited.updatedAt = Date(timeIntervalSinceReferenceDate: 50)
        let untouched = SyncedSettingsModel()
        store.context.insert(untouched)
        store.context.insert(edited)
        try store.context.save()

        try store.dedupe()
        #expect(try store.context.fetchCount(FetchDescriptor<SyncedSettingsModel>()) == 1)
        #expect(try store.syncedSettings().typingSpeedWPM == 90)
    }

    @Test func deviceIDFollowsHardware() throws {
        let store = try KudzuStore.inMemory()
        let original = try store.deviceID()

        #expect(try store.reconcileDevice(hardwareID: "   ") == original)
        #expect(try store.localSettings().hardwareID == "")

        #expect(try store.reconcileDevice(hardwareID: "mac-a") == original)
        #expect(try store.localSettings().hardwareID == "mac-a")
        #expect(try store.reconcileDevice(hardwareID: "mac-a") == original)

        let moved = try store.reconcileDevice(hardwareID: "mac-b")
        #expect(moved != original)
        #expect(try store.deviceID() == moved)
        #expect(try store.localSettings().hardwareID == "mac-b")
        #expect(try store.localSettings().deviceID == moved)
    }

    @Test func settingsDefaultsAndUpdates() throws {
        let store = try KudzuStore.inMemory()
        let settings = try store.syncedSettings()
        #expect(settings.typingSpeedWPM == 40)
        #expect(settings.defaultTriggerPrefix == ";")
        #expect(settings.updatedAt == nil)
        try store.updateSyncedSettings { $0.typingSpeedWPM = 60 }
        #expect(try store.syncedSettings().typingSpeedWPM == 60)
        #expect(try store.syncedSettings().updatedAt != nil)

        let local = try store.localSettings()
        #expect(local.syncEnabled)
        #expect(local.paletteHotkey == nil)
    }

    @Test func statsSummaryUsesSyncedWPM() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "s", triggers: [";s"], content: "S")
        try store.upsert(snippet)
        try store.recordUsage(snippetID: snippet.id, charsSaved: 1000)
        try store.updateSyncedSettings { $0.typingSpeedWPM = 100 }
        let summary = try store.statsSummary()
        #expect(summary.totalExpansions == 1)
        #expect(summary.estimatedTimeSaved == 120)
    }
}
