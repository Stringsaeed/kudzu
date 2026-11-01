import Foundation

/// The reason a backup snapshot was created.
public enum BackupReason: String, Codable, Hashable, Sendable, CaseIterable {
    case daily
    case preImport
    case manual
}

/// Metadata describing a stored backup file on disk.
public struct BackupInfo: Codable, Hashable, Sendable {
    public var url: URL
    public var filename: String
    public var createdAt: Date
    public var reason: BackupReason

    public init(url: URL, filename: String, createdAt: Date, reason: BackupReason) {
        self.url = url
        self.filename = filename
        self.createdAt = createdAt
        self.reason = reason
    }
}

/// Manages local JSON backup snapshots of a `KudzuLibrary`.
public struct BackupStore: @unchecked Sendable {
    public let directory: URL
    public let fileManager: FileManager
    public let calendar: Calendar

    /// Creates a backup store rooted at `directory`.
    public init(
        directory: URL,
        fileManager: FileManager = .default,
        calendar: Calendar = .current
    ) {
        self.directory = directory
        self.fileManager = fileManager
        self.calendar = calendar
    }

    /// Writes a snapshot of `library` named `kudzu-backup-<yyyy-MM-dd'T'HHmmss>-<reason>.json`.
    @discardableResult
    public func write(
        _ library: KudzuLibrary,
        reason: BackupReason,
        now: Date = .now
    ) throws -> URL {
        try fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
        let timestamp = formatTimestamp(now)
        let filename = "kudzu-backup-\(timestamp)-\(reason.rawValue).json"
        let fileURL = directory.appendingPathComponent(filename, isDirectory: false)
        let data = try KudzuJSON.encode(library)
        try data.write(to: fileURL, options: .atomic)
        return fileURL
    }

    /// Lists all valid backup snapshots in `directory`, sorted newest first.
    public func list() throws -> [BackupInfo] {
        guard fileManager.fileExists(atPath: directory.path) else {
            return []
        }
        let urls = try fileManager.contentsOfDirectory(
            at: directory,
            includingPropertiesForKeys: nil,
            options: [.skipsHiddenFiles]
        )
        var infos: [BackupInfo] = []
        for url in urls {
            if let info = parseBackupInfo(from: url) {
                infos.append(info)
            }
        }
        infos.sort { lhs, rhs in
            if lhs.createdAt != rhs.createdAt {
                return lhs.createdAt > rhs.createdAt
            }
            return lhs.filename > rhs.filename
        }
        return infos
    }

    /// Loads and decodes a `KudzuLibrary` from a backup entry.
    public func load(_ info: BackupInfo) throws -> KudzuLibrary {
        let data = try Data(contentsOf: info.url)
        return try KudzuJSON.decode(data)
    }

    /// Loads and decodes a `KudzuLibrary` directly from a backup file URL.
    public func load(_ url: URL) throws -> KudzuLibrary {
        let data = try Data(contentsOf: url)
        return try KudzuJSON.decode(data)
    }

    /// Deletes older backups so that at most `keep` newest backups remain.
    @discardableResult
    public func prune(keep: Int = 14) throws -> [BackupInfo] {
        let all = try list()
        let keepCount = max(0, keep)
        guard all.count > keepCount else { return [] }
        let toDelete = Array(all.dropFirst(keepCount))
        for info in toDelete {
            try fileManager.removeItem(at: info.url)
        }
        return toDelete
    }

    /// Returns `true` if no `.daily` backup exists yet on the calendar day of `now`.
    public func needsDailyBackup(now: Date = .now, calendar: Calendar? = nil) -> Bool {
        let cal = calendar ?? self.calendar
        guard let all = try? list() else { return true }
        return !all.contains { info in
            info.reason == .daily && cal.isDate(info.createdAt, inSameDayAs: now)
        }
    }

    // MARK: - Timestamp helpers

    private func formatTimestamp(_ date: Date) -> String {
        let comps = calendar.dateComponents([.year, .month, .day, .hour, .minute, .second], from: date)
        let y = comps.year ?? 1970
        let m = comps.month ?? 1
        let d = comps.day ?? 1
        let hh = comps.hour ?? 0
        let mm = comps.minute ?? 0
        let ss = comps.second ?? 0
        return String(format: "%04d-%02d-%02dT%02d%02d%02d", y, m, d, hh, mm, ss)
    }

    private func parseBackupInfo(from url: URL) -> BackupInfo? {
        let filename = url.lastPathComponent
        let prefix = "kudzu-backup-"
        let suffix = ".json"
        guard filename.hasPrefix(prefix), filename.hasSuffix(suffix) else {
            return nil
        }
        let body = String(filename.dropFirst(prefix.count).dropLast(suffix.count))
        guard let lastDash = body.lastIndex(of: "-") else {
            return nil
        }
        let timestampPart = String(body[..<lastDash])
        let reasonPart = String(body[body.index(after: lastDash)...])
        guard let reason = BackupReason(rawValue: reasonPart),
              let date = parseTimestamp(timestampPart) else {
            return nil
        }
        return BackupInfo(url: url, filename: filename, createdAt: date, reason: reason)
    }

    private func parseTimestamp(_ text: String) -> Date? {
        // Expected format: yyyy-MM-dd'T'HHmmss (17 characters)
        let chars = Array(text)
        guard chars.count == 17,
              chars[4] == "-",
              chars[7] == "-",
              chars[10] == "T" else {
            return nil
        }
        guard let year = Int(String(chars[0...3])),
              let month = Int(String(chars[5...6])),
              let day = Int(String(chars[8...9])),
              let hour = Int(String(chars[11...12])),
              let minute = Int(String(chars[13...14])),
              let second = Int(String(chars[15...16])) else {
            return nil
        }
        var comps = DateComponents()
        comps.calendar = calendar
        comps.timeZone = calendar.timeZone
        comps.year = year
        comps.month = month
        comps.day = day
        comps.hour = hour
        comps.minute = minute
        comps.second = second
        return calendar.date(from: comps)
    }
}
