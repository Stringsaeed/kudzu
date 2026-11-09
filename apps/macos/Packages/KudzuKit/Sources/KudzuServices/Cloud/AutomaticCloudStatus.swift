import Foundation
import CloudKit
import CoreData
import KudzuCore

/// Observes framework events; never promises a manual CloudKit flush.
public final class AutomaticCloudStatus: @unchecked Sendable {
    private let lock = NSLock()
    private var status: SyncStatus = .starting
    private var restartReason: String?
    private var observers: [NSObjectProtocol] = []
    private let enabled: Bool
    private let available: Bool
    private let publish: @Sendable (SyncStatus) -> Void
    public init(enabled: Bool, available: Bool, publish: @escaping @Sendable (SyncStatus) -> Void) {
        self.enabled = enabled; self.available = available; self.publish = publish
        if available {
            observers.append(NotificationCenter.default.addObserver(forName: NSPersistentCloudKitContainer.eventChangedNotification, object: nil, queue: nil) { [weak self] note in
                guard let self, let event = note.userInfo?[NSPersistentCloudKitContainer.eventNotificationUserInfoKey] as? NSPersistentCloudKitContainer.Event else { return }
                if let error = event.error { self.set(.error(message: error.localizedDescription)) }
                else if event.endDate == nil { self.set(.syncing) }
                else { self.set(.unavailable(reason: "Automatic iCloud activity completed; delivery to other Macs is not confirmed.")) }
            })
            observers.append(NotificationCenter.default.addObserver(forName: .CKAccountChanged, object: nil, queue: nil) { [weak self] _ in
                self?.requireRestart("iCloud account changed. Restart Kudzu before continuing cloud work.")
            })
        }
    }
    deinit { stop() }
    public var currentStatus: SyncStatus { lock.withLock { status } }
    private func set(_ value: SyncStatus) {
        let next = lock.withLock { () -> SyncStatus in
            let next = restartReason.map { SyncStatus.error(message: $0) } ?? value
            status = next
            return next
        }
        publish(next)
    }
    private func requireRestart(_ reason: String) {
        lock.withLock { restartReason = reason }
        set(.error(message: reason))
    }
    public func configurationChanged() { requireRestart("Restart Kudzu to apply the iCloud setting.") }
    public func recheck() {
        if let reason = lock.withLock({ restartReason }) { set(.error(message: reason)); return }
        guard enabled else { set(.off); return }
        guard available else { set(.unavailable(reason: "This build is local-only; fresh iCloud provisioning is required.")); return }
        set(.starting)
        CKContainer(identifier: LibraryStoreConfiguration.destinationContainerIdentifier).accountStatus { [weak self] account, error in
            if let error { self?.set(.error(message: error.localizedDescription)) }
            else if account != .available { self?.set(.noAccount(reason: "Sign in to iCloud in System Settings.")) }
            else { self?.set(.unavailable(reason: "iCloud sync runs automatically; waiting for framework activity.")) }
        }
    }
    public func stop() { for observer in observers { NotificationCenter.default.removeObserver(observer) }; observers = [] }
}
