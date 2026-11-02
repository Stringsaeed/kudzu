import Foundation
import Testing
@testable import KudzuCore

@Suite struct CloudSyncTests {
    @Test func identifiersStayPinned() {
        #expect(KudzuStoreConstants.cloudKitContainerID == "iCloud.sh.saeed.kudzu")
        #expect(KudzuStoreConstants.cloudKitTeamID == "V3HN8HXZYK")
    }

    @Test func eligibility() {
        let container = KudzuStoreConstants.cloudKitContainerID
        let team = KudzuStoreConstants.cloudKitTeamID
        #expect(CloudSyncEligibility.block(teamID: nil, containerIdentifiers: [container]) == .unsignedBuild)
        #expect(CloudSyncEligibility.block(teamID: "", containerIdentifiers: [container]) == .unsignedBuild)
        #expect(CloudSyncEligibility.block(teamID: "OTHERTEAM", containerIdentifiers: [container]) == .differentTeam)
        #expect(CloudSyncEligibility.block(teamID: team, containerIdentifiers: []) == .missingEntitlement)
        #expect(CloudSyncEligibility.block(teamID: team, containerIdentifiers: ["iCloud.other"]) == .missingEntitlement)
        #expect(CloudSyncEligibility.block(teamID: team, containerIdentifiers: [container]) == nil)
    }

    @Test func preferenceDefaultsOnUntilSet() {
        let name = "kudzu-sync-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defaults.removePersistentDomain(forName: name)
        defer { defaults.removePersistentDomain(forName: name) }

        #expect(SyncPreference.isEnabled(in: defaults))
        SyncPreference.setEnabled(false, in: defaults)
        #expect(!SyncPreference.isEnabled(in: defaults))
        SyncPreference.setEnabled(true, in: defaults)
        #expect(SyncPreference.isEnabled(in: defaults))
    }

    @Test func phasePrefersBlockThenOpenFailureThenRestart() {
        let blocked = SyncSnapshot(preferenceEnabled: true, runningEnabled: false, blocked: .unsignedBuild)
        #expect(blocked.phase == .unavailable(.unsignedBuild))

        let failedOpen = SyncSnapshot(
            preferenceEnabled: true, runningEnabled: false, openError: "container refused"
        )
        #expect(failedOpen.phase == .failed("container refused"))

        let turningOn = SyncSnapshot(preferenceEnabled: true, runningEnabled: false)
        #expect(turningOn.phase == .needsRestart(turningOn: true))
        #expect(turningOn.phase.statusText == "iCloud sync turns on after you quit and reopen Kudzu")

        let turningOff = SyncSnapshot(preferenceEnabled: false, runningEnabled: true)
        #expect(turningOff.phase == .needsRestart(turningOn: false))

        let off = SyncSnapshot(preferenceEnabled: false, runningEnabled: false)
        #expect(off.phase == .off)
        #expect(off.phase.statusText == "iCloud sync is off")
    }

    @Test func phaseWhileRunning() {
        let signedOut = SyncSnapshot(preferenceEnabled: true, runningEnabled: true, account: .noAccount)
        #expect(signedOut.phase == .noAccount)

        let restricted = SyncSnapshot(preferenceEnabled: true, runningEnabled: true, account: .restricted)
        #expect(restricted.phase == .restricted)

        let waiting = SyncSnapshot(preferenceEnabled: true, runningEnabled: true, account: .temporarilyUnavailable)
        #expect(waiting.phase == .temporarilyUnavailable)

        let rejected = SyncSnapshot(
            preferenceEnabled: true, runningEnabled: true, accountError: "missing entitlement", account: .unknown
        )
        #expect(rejected.phase == .failed("missing entitlement"))

        let busy = SyncSnapshot(
            preferenceEnabled: true, runningEnabled: true, account: .available, activities: [.importing, .exporting]
        )
        #expect(busy.phase == .syncing([.importing, .exporting]))
        #expect(busy.phase.statusText == "Syncing with iCloud…")

        let fresh = SyncSnapshot(preferenceEnabled: true, runningEnabled: true, account: .available)
        #expect(fresh.phase == .upToDate(lastSyncedAt: nil))
        #expect(fresh.phase.statusText == "iCloud sync is on")

        let broken = SyncSnapshot(
            preferenceEnabled: true, runningEnabled: true, account: .available, lastError: "network down"
        )
        #expect(broken.phase == .failed("network down"))
    }

    @Test func eventsMoveTheSnapshot() {
        let start = Date(timeIntervalSinceReferenceDate: 1_000)
        var snapshot = SyncSnapshot(preferenceEnabled: true, runningEnabled: true, account: .available)

        snapshot = SyncSnapshotReducer.apply(
            CloudSyncEvent(kind: .import, finished: false, succeeded: false, errorDescription: nil, date: start),
            to: snapshot
        )
        #expect(snapshot.phase == .syncing([.importing]))
        #expect(snapshot.phase.statusText == "Downloading snippet changes…")

        snapshot = SyncSnapshotReducer.apply(
            CloudSyncEvent(kind: .export, finished: false, succeeded: false, errorDescription: nil, date: start),
            to: snapshot
        )
        #expect(snapshot.activities == [.importing, .exporting])

        let failedAt = start.addingTimeInterval(5)
        snapshot = SyncSnapshotReducer.apply(
            CloudSyncEvent(kind: .import, finished: true, succeeded: false, errorDescription: " ", date: failedAt),
            to: snapshot
        )
        #expect(snapshot.activities == [.exporting])
        #expect(snapshot.phase == .syncing([.exporting]))
        #expect(snapshot.lastError == "iCloud sync failed.")

        let exportedAt = failedAt.addingTimeInterval(5)
        snapshot = SyncSnapshotReducer.apply(
            CloudSyncEvent(kind: .export, finished: true, succeeded: true, errorDescription: nil, date: exportedAt),
            to: snapshot
        )
        #expect(snapshot.activities.isEmpty)
        #expect(snapshot.lastError == nil)
        #expect(snapshot.phase == .upToDate(lastSyncedAt: exportedAt))
        #expect(snapshot.phase.statusText.hasPrefix("Synced with iCloud "))
    }

    @Test func olderSuccessDoesNotHideANewerFailure() {
        let failure = Date(timeIntervalSinceReferenceDate: 50)
        var snapshot = SyncSnapshot(
            preferenceEnabled: true, runningEnabled: true, account: .available,
            lastError: "import failed", lastErrorAt: failure
        )
        snapshot = SyncSnapshotReducer.apply(
            CloudSyncEvent(kind: .setup, finished: true, succeeded: true, errorDescription: nil, date: failure.addingTimeInterval(-10)),
            to: snapshot
        )
        #expect(snapshot.phase == .failed("import failed"))
        #expect(snapshot.lastSuccessAt == failure.addingTimeInterval(-10))
    }

    @Test func onlyAFinishedImportAsksForDedupe() {
        let date = Date(timeIntervalSinceReferenceDate: 0)
        let started = CloudSyncEvent(kind: .import, finished: false, succeeded: false, errorDescription: nil, date: date)
        let imported = CloudSyncEvent(kind: .import, finished: true, succeeded: true, errorDescription: nil, date: date)
        let failed = CloudSyncEvent(kind: .import, finished: true, succeeded: false, errorDescription: "no", date: date)
        let exported = CloudSyncEvent(kind: .export, finished: true, succeeded: true, errorDescription: nil, date: date)
        #expect(!started.shouldNoteRemoteChange)
        #expect(imported.shouldNoteRemoteChange)
        #expect(!failed.shouldNoteRemoteChange)
        #expect(!exported.shouldNoteRemoteChange)
    }
}
