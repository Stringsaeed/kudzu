import CloudKit
import CoreData
import Foundation
import KudzuCore
import Observation

/// Turns CloudKit account and container events into `SyncSnapshot` for the menu bar and Settings.
@MainActor @Observable
final class SyncMonitor {
    private(set) var snapshot: SyncSnapshot

    @ObservationIgnored private var store: KudzuStore?
    @ObservationIgnored private let observers = ObserverTokens()
    @ObservationIgnored private var started = false

    init(snapshot: SyncSnapshot) {
        self.snapshot = snapshot
    }

    var statusText: String { snapshot.phase.statusText }

    /// Begins account checks and event observation when this process is actually mirroring.
    func start(store: KudzuStore) {
        guard !started else { return }
        started = true
        self.store = store
        guard snapshot.runningEnabled, snapshot.blocked == nil, snapshot.openError == nil else { return }

        observers.add(NotificationCenter.default.addObserver(
            forName: NSPersistentCloudKitContainer.eventChangedNotification,
            object: nil, queue: .main
        ) { [weak self] note in
            guard let mapped = Self.event(from: note) else { return }
            MainActor.assumeIsolated { self?.apply(mapped) }
        })
        observers.add(NotificationCenter.default.addObserver(
            forName: .CKAccountChanged, object: nil, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.refreshAccount() }
        })
        refreshAccount()
    }

    func setPreference(_ enabled: Bool) {
        snapshot.preferenceEnabled = enabled
    }

    private func apply(_ event: CloudSyncEvent) {
        snapshot = SyncSnapshotReducer.apply(event, to: snapshot)
        if event.shouldNoteRemoteChange {
            store?.noteRemoteChange()
        }
    }

    /// Reads the event off the notification before hopping to the main actor. `Notification` isn't Sendable.
    nonisolated private static func event(from note: Notification) -> CloudSyncEvent? {
        guard let event = note.userInfo?[NSPersistentCloudKitContainer.eventNotificationUserInfoKey]
            as? NSPersistentCloudKitContainer.Event else { return nil }
        return CloudSyncEvent(
            kind: kind(event.type),
            finished: event.endDate != nil,
            succeeded: event.succeeded,
            errorDescription: event.error?.localizedDescription,
            date: event.endDate ?? event.startDate
        )
    }

    private func refreshAccount() {
        let container = CKContainer(identifier: KudzuStoreConstants.cloudKitContainerID)
        container.accountStatus { [weak self] status, error in
            let update = Self.accountUpdate(status: status, error: error)
            Task { @MainActor in
                guard let self else { return }
                self.snapshot.account = update.account
                self.snapshot.accountError = update.accountError
            }
        }
    }

    nonisolated private static func kind(_ type: NSPersistentCloudKitContainer.EventType) -> CloudSyncEvent.Kind {
        switch type {
        case .setup: .setup
        case .import: .import
        case .export: .export
        @unknown default: .setup
        }
    }

    nonisolated private static func accountUpdate(status: CKAccountStatus, error: Error?) -> (account: SyncAccountState, accountError: String?) {
        if let ck = error as? CKError {
            switch ck.code {
            case .missingEntitlement, .badContainer, .permissionFailure:
                return (.unknown, "iCloud rejected this build (\(ck.localizedDescription)).")
            default:
                break
            }
        }
        switch status {
        case .available: return (.available, nil)
        case .noAccount: return (.noAccount, nil)
        case .restricted: return (.restricted, nil)
        case .temporarilyUnavailable: return (.temporarilyUnavailable, nil)
        case .couldNotDetermine:
            return (.unknown, error?.localizedDescription)
        @unknown default:
            return (.unknown, nil)
        }
    }
}

/// Notification tokens removed when the monitor is released.
private final class ObserverTokens: @unchecked Sendable {
    private var values: [any NSObjectProtocol] = []

    func add(_ token: any NSObjectProtocol) { values.append(token) }

    deinit {
        for token in values { NotificationCenter.default.removeObserver(token) }
    }
}
