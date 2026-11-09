// Ported from kudzu-core/src/interchange/backup.rs
// Local JSON backup snapshots of a [`KudzuLibrary`].
//
// Files are named `kudzu-backup-<yyyy-MM-dd'T'HHmmss>-<reason>[.N].json` in local time, exactly
// like the Swift app, so either app can list the other's backups.

import Foundation

/// Local JSON snapshots, named identically to Tauri backups. Never overwrites a safety copy.
public struct BackupStore: Sendable {
    public var dir: URL
    public var timeZone: TimeZone
    public init(dir: URL, timeZone: TimeZone = .current) { self.dir = dir; self.timeZone = timeZone }
    func formatter() -> DateFormatter {
        let f = DateFormatter(); f.locale = Locale(identifier:"en_US_POSIX"); f.timeZone = timeZone; f.dateFormat = "yyyy-MM-dd'T'HHmmss"; f.isLenient = false; return f
    }
    /// A second backup of the same reason in one second gets .2, .3, ... atomically.
    public func write(_ library: Library, reason: BackupReason, now: Date) throws -> BackupInfo {
        try FileManager.default.createDirectory(at:dir,withIntermediateDirectories:true)
        let data = Data(try KudzuJSON.encodeLibrary(library).utf8), f = formatter(), timestamp = f.string(from:now)
        var sequence = 1
        while true {
            let filename = "kudzu-backup-\(timestamp)-\(reason.rawValue)\(sequence == 1 ? "" : ".\(sequence)").json", path = dir.appendingPathComponent(filename)
            do {
                try data.write(to:path,options:.withoutOverwriting)
                let file = try FileHandle(forWritingTo:path)
                do { try file.synchronize(); try file.close() }
                catch { try? file.close(); throw error }
                return BackupInfo(path:path.path,filename:filename,createdAt:f.date(from:timestamp) ?? now,reason:reason)
            } catch let error as CocoaError where error.code == .fileWriteFileExists { sequence += 1 }
        }
    }
    /// All valid backups in the directory, newest first.
    public func list() throws -> [BackupInfo] {
        guard FileManager.default.fileExists(atPath:dir.path) else { return [] }
        let f = formatter()
        let infos = try FileManager.default.contentsOfDirectory(at:dir,includingPropertiesForKeys:nil).compactMap { url -> BackupInfo? in
            let name = url.lastPathComponent
            guard name.hasPrefix("kudzu-backup-"), name.hasSuffix(".json") else { return nil }
            let body = String(name.dropFirst(13).dropLast(5))
            guard let sep = body.lastIndex(of:"-") else { return nil }
            let timestamp = String(body[..<sep]), suffix = String(body[body.index(after:sep)...])
            guard timestamp.count == 17, let date = f.date(from:timestamp), let reason = BackupReason(rawValue:suffix.components(separatedBy:".")[0]) else { return nil }
            return BackupInfo(path:dir.appendingPathComponent(name).path,filename:name,createdAt:date,reason:reason)
        }
        return infos.sorted {
            if $0.createdAt != $1.createdAt { return $0.createdAt > $1.createdAt }
            let a = sequence($0.filename), b = sequence($1.filename)
            return a != b ? a > b : $0.filename > $1.filename
        }
    }
    func sequence(_ filename: String) -> UInt32 { UInt32(filename.dropLast(5).components(separatedBy:".").last ?? "") ?? 1 }
    /// Loads and decodes a backup file.
    public func load(_ path: URL) throws -> Library { try KudzuJSON.decodeLibrary(String(contentsOf:path,encoding:.utf8)) }
    /// Deletes older backups so at most `keep` (the app uses 14) newest remain; returns those deleted.
    @discardableResult public func prune(keep: Int = 14) throws -> [BackupInfo] {
        let gone = Array(try list().dropFirst(max(0,keep)))
        for info in gone { try FileManager.default.removeItem(atPath:info.path) }; return gone
    }
    /// True if no `daily` backup exists yet on the local calendar day of `now`.
    public func needsDailyBackup(now: Date) -> Bool {
        var c = Calendar(identifier:.gregorian); c.timeZone = timeZone
        return (try? list().contains { $0.reason == .daily && c.isDate($0.createdAt,inSameDayAs:now) }) != true
    }
}
