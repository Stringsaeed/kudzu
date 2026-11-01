import Foundation
import KudzuCore
import Observation
import os

/// Runs automatic daily library backups on launch and every hour, keeping the last 14 snapshots.
@MainActor @Observable
final class BackupScheduler {
    static let keepCount = 14
    static let checkInterval: Duration = .seconds(3600)

    let store: KudzuStore
    let backupStore: BackupStore

    private(set) var lastBackupAt: Date?
    private(set) var lastError: String?

    @ObservationIgnored private var loopTask: Task<Void, Never>?
    private static let log = Logger(subsystem: "sh.saeed.kudzu", category: "backups")

    /// Default backup directory in Application Support (`.../Application Support/Kudzu/Backups`).
    static func defaultBackupDirectory(fileManager: FileManager = .default) -> URL {
        let appSupport = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL.applicationSupportDirectory
        return appSupport
            .appendingPathComponent(KudzuStoreConstants.defaultDirectoryName, isDirectory: true)
            .appendingPathComponent("Backups", isDirectory: true)
    }

    init(store: KudzuStore, backupStore: BackupStore? = nil) {
        self.store = store
        self.backupStore = backupStore ?? BackupStore(directory: Self.defaultBackupDirectory())
    }

    /// Runs an immediate daily backup check and schedules hourly checks. Safe to call repeatedly.
    func start() {
        guard loopTask == nil else { return }
        runDailyBackupIfNeeded()
        loopTask = Task { @MainActor [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: Self.checkInterval)
                guard !Task.isCancelled else { break }
                self?.runDailyBackupIfNeeded()
            }
        }
    }

    /// Stops the background hourly check task.
    func stop() {
        loopTask?.cancel()
        loopTask = nil
    }

    /// Writes a `.daily` backup and prunes to the newest 14 snapshots if no daily backup exists today.
    @discardableResult
    func runDailyBackupIfNeeded(now: Date = .now) -> URL? {
        guard backupStore.needsDailyBackup(now: now) else { return nil }
        do {
            let library = try store.library()
            let url = try backupStore.write(library, reason: .daily, now: now)
            _ = try backupStore.prune(keep: Self.keepCount)
            lastBackupAt = now
            lastError = nil
            return url
        } catch {
            Self.log.error("Daily backup failed: \(error.localizedDescription)")
            lastError = error.localizedDescription
            return nil
        }
    }
}
