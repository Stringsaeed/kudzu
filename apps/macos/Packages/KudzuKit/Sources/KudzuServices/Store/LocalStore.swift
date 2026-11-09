import Foundation
import KudzuCore

/// Device-local state only. The library accessor reads the repository, never a JSON mirror.
public final class LocalStore: @unchecked Sendable {
    public struct Contents: Sendable {
        public var settings: Settings
        public var formMemory: FormMemory
    }
    public let dir: URL
    public let deviceID: String
    public let firstRun: Bool
    public let repository: LibraryRepository
    private let lock = NSLock()
    private var contents: Contents
    private let blockedWrites = Set<String>()
    private var pendingWrites: [String: @Sendable () throws -> (Data, mode_t)] = [:]
    private let io = DispatchQueue(label: "sh.saeed.kudzu.local-store")
    private let log = Logging.logger(category: "store")
    public init(dir: URL, repository: LibraryRepository, defaults: Settings, firstRun: Bool) throws {
        self.dir = dir; self.repository = repository; self.firstRun = firstRun
        let fm = FileManager.default
        func read<T>(_ name: String, fallback: T, decode: (Data) throws -> T) throws -> T {
            let url = dir.appendingPathComponent(name)
            return fm.fileExists(atPath: url.path) ? try decode(Data(contentsOf: url)) : fallback
        }
        let settings = try read("settings.json", fallback: defaults) { try KudzuJSON.decoder(settingsDefaults: defaults).decode(Settings.self, from: $0) }
        let memory = try read("form-memory.json", fallback: FormMemory()) { try JSONDecoder().decode(FormMemory.self, from: $0) }
        contents = Contents(settings: settings, formMemory: memory)
        let url = dir.appendingPathComponent("device-id")
        if fm.fileExists(atPath: url.path) {
            deviceID = try String(contentsOf: url, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
            guard !deviceID.isEmpty else { throw ServiceError("Empty device identity; original files retained") }
        } else {
            deviceID = UUID().lowercased
            try AtomicFile.write(Data(deviceID.utf8), to: url, mode: 0o600)
        }
    }
    @discardableResult public func withLock<T>(_ body: (inout Contents) throws -> T) rethrows -> T {
        try lock.withLock { try body(&contents) }
    }
    public var library: Library { repository.snapshot }
    public var settings: Settings { withLock { $0.settings } }
    public var formMemory: FormMemory { withLock { $0.formMemory } }
    public var backups: BackupStore { BackupStore(dir: dir.appendingPathComponent("backups")) }
    public func saveSettings() {
        save("settings.json") { c in
            let settings = c.settings
            return { (try KudzuJSON.encoder(pretty: true).encode(settings), 0o644) }
        }
    }

    public func saveFormMemory() {
        save("form-memory.json") { c in
            let memory = c.formMemory
            return { (try JSONEncoder().encode(memory), 0o644) }
        }
    }

    /// Drops remembered values for snippets that are not in the last durable library.
    /// The retained ids and the edit share one lock, so a newer deletion that exists only in
    /// memory is not treated as committed. The file is saved after that lock opens.
    public func pruneFormMemoryToCommittedLibrary() {
        let retained = Set(library.snippets.map(\.id))
        withLock { c in
            c.formMemory.retainSnippets(retained)
        }
        saveFormMemory()
    }

    /// Queues the newest snapshot of `name`. Off the main thread, also waits until it's on disk.
    private func save(_ name: String, snapshot: (Contents) -> @Sendable () throws -> (Data, mode_t)) {
        guard !blockedWrites.contains(name) else { return }
        let schedule = withLock { c -> Bool in
            // Only one queued write per file: later saves replace its snapshot.
            pendingWrites.updateValue(snapshot(c), forKey: name) == nil
        }
        if schedule { io.async { self.writePending(name) } }
        if !Thread.isMainThread { flush() }
    }

    /// Runs on `io`: writes the newest snapshot of `name`, if one is still waiting.
    private func writePending(_ name: String) {
        guard let encode = withLock({ _ in pendingWrites.removeValue(forKey: name) }) else { return }
        do {
            let (data, mode) = try encode()
            try AtomicFile.write(data, to: dir.appendingPathComponent(name), mode: mode)
        } catch {
            log.error("saving \(name) failed: \(error.localizedDescription)")
        }
    }

    /// Waits until every queued write is on disk.
    public func flush() { io.sync {} }


    public func noteAppVersion(_ version: String) throws {
        let marker = dir.appendingPathComponent("native-app")
        let previous = settings.lastRunVersion
        if (!firstRun && !FileManager.default.fileExists(atPath: marker.path)) || (previous != nil && previous != version) {
            _ = try backups.write(library, reason: .preUpdate, now: Date())
        }
        try AtomicFile.write(Data(version.utf8), to: marker)
        withLock { $0.settings.lastRunVersion = version }
        saveSettings()
    }

    public func dailyBackup() {
        guard backups.needsDailyBackup(now: Date()) else { return }
        do { _ = try backups.write(library, reason: .daily, now: Date()); _ = try backups.prune(keep: 14) }
        catch { log.error("backup failed: \(error.localizedDescription)") }
    }
}
