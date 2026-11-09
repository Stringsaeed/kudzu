import Foundation
import Darwin
import CryptoKit
import CoreData
import KudzuCore

/// Runs before local writers/services. Receipt and recovery evidence are device-local.
public enum LibraryMigration {
    public enum Role: String, Codable, Sendable { case primary, join }
    public enum Stage: Sendable { case copied, imported, verified, activating }
    struct Receipt: Codable { var version = 1; var role: Role; var recovery: String; var store: String }
    struct RecoveryManifest: Codable { var files: [String: String]; var omittedRuntimeFiles: [String] }

    public static func open(directory: URL, defaultsDomain: String?, role: Role = .primary,
                            legacyDirectory: URL? = nil,
                            fault: (@Sendable (Stage) throws -> Void)? = nil) async throws -> (repository: LibraryRepository, firstRun: Bool) {
        let fm = FileManager.default
        try fm.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let fd = Darwin.open(directory.appendingPathComponent("migration.lock").path, O_CREAT | O_RDWR | O_CLOEXEC, 0o600)
        guard fd >= 0 else { throw ServiceError("Cannot open migration lock") }
        defer { flock(fd, LOCK_UN); close(fd) }
        guard flock(fd, LOCK_EX | LOCK_NB) == 0 else { throw ServiceError("Another Kudzu is migrating this directory. Retry after it exits.") }
        let marker = directory.appendingPathComponent("active-library.json")
        if fm.fileExists(atPath: marker.path) {
            let receipt = try JSONDecoder().decode(Receipt.self, from: Data(contentsOf: marker))
            guard receipt.version == 1, receipt.store == "Library.store" else { throw ServiceError("Unknown active library receipt; originals retained") }
            let url = directory.appendingPathComponent(receipt.store)
            guard fm.fileExists(atPath: url.path) else { throw ServiceError("Active library is missing; restore its recovery copy before retrying") }
            try validateInitializedStore(url)
            return (try await LibraryRepository.open(.local(url: url)), false)
        }
        if let legacyDirectory,
           legacyDirectory.standardizedFileURL != directory.standardizedFileURL,
           !fm.fileExists(atPath: directory.appendingPathComponent("library.json").path),
           fm.fileExists(atPath: legacyDirectory.appendingPathComponent("library.json").path) {
            var omitted: [String] = []
            // Copy library.json last: a failed copy must not make a partial legacy transfer
            // look complete on retry. Existing destination files retain precedence.
            let entries = try fm.contentsOfDirectory(atPath: legacyDirectory.path).sorted {
                if $0 == "library.json" { return false }
                if $1 == "library.json" { return true }
                return $0 < $1
            }
            for entry in entries {
                try copyRecoveryEntry(from: legacyDirectory.appendingPathComponent(entry),
                                      to: directory.appendingPathComponent(entry), relative: entry,
                                      onlyMissing: true, omitted: &omitted)
            }
        }
        let pendingURL = directory.appendingPathComponent("pending-library-migration")
        let ownsCandidate = fm.fileExists(atPath: pendingURL.path) && (try? String(contentsOf: pendingURL, encoding: .utf8)) == "1"
        let sourceURL = directory.appendingPathComponent("library.json")
        let entries = try fm.contentsOfDirectory(atPath: directory.path)
        let candidateFiles: Set<String> = ["Library.store", "Library.store-wal", "Library.store-shm", ".Library_SUPPORT"]
        let sourceExists = fm.fileExists(atPath: sourceURL.path)
        let firstRun = !sourceExists && entries.allSatisfy { $0 == "migration.lock" || $0 == "pending-library-migration" || $0.hasPrefix("migration-recovery-") || (ownsCandidate && candidateFiles.contains($0)) }
        guard ownsCandidate || !entries.contains(where: { candidateFiles.contains($0) }) else { throw ServiceError("Unrecognized destination store; refusing to replace it. Originals retained.") }
        guard sourceExists || firstRun else { throw ServiceError("Library source is missing in an existing data directory; originals retained") }
        let recovery = directory.appendingPathComponent("migration-recovery-" + UUID().uuidString)
        try fm.createDirectory(at: recovery, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
        var manifest: [String: String] = [:]
        var omitted: [String] = []
        for entry in entries where entry != "migration.lock" && !entry.hasPrefix("migration-recovery-") && !candidateFiles.contains(entry) {
            let from = directory.appendingPathComponent(entry)
            try copyRecoveryEntry(from: from, to: recovery.appendingPathComponent(entry),
                                  relative: entry, onlyMissing: false, omitted: &omitted)
        }
        if let defaultsDomain, let domain = UserDefaults.standard.persistentDomain(forName: defaultsDomain) {
            let data = try PropertyListSerialization.data(fromPropertyList: domain, format: .binary, options: 0)
            try AtomicFile.write(data, to: recovery.appendingPathComponent("ui-defaults.plist"), mode: 0o600)
        }
        let enumerator = fm.enumerator(at: recovery, includingPropertiesForKeys: [.isRegularFileKey])!
        for url in enumerator.allObjects.compactMap({ $0 as? URL }) {
            if try entryType(url) == S_IFREG {
                let relative = String(url.path.dropFirst(recovery.path.count + 1))
                manifest[relative] = SHA256.hash(data: try Data(contentsOf: url)).map { String(format: "%02x", $0) }.joined()
                try fm.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
                let original = directory.appendingPathComponent(relative)
                if fm.fileExists(atPath: original.path) {
                    guard try Data(contentsOf: original) == Data(contentsOf: url) else { throw ServiceError("Recovery copy differs: \(relative)") }
                }
            }
        }
        try AtomicFile.write(try JSONEncoder().encode(RecoveryManifest(files: manifest, omittedRuntimeFiles: omitted)), to: recovery.appendingPathComponent("manifest.json"), mode: 0o600)
        let sourceBytes = sourceExists ? try Data(contentsOf: recovery.appendingPathComponent("library.json")) : nil
        try fault?(.copied)
        var source = try sourceBytes.map { try KudzuJSON.decode(Library.self, from: $0) } ?? Library.empty
        if firstRun && role == .primary { Coordinator.ensureGettingStartedSnippets(&source, now: Date()) }
        if role == .join { source = .empty }
        let destination = directory.appendingPathComponent("Library.store")
        try AtomicFile.write(Data("1".utf8), to: pendingURL, mode: 0o600)
        // No active receipt: an interrupted candidate is never authoritative. Rebuild it.
        for entry in try fm.contentsOfDirectory(atPath: directory.path) where candidateFiles.contains(entry) {
            try fm.removeItem(at: directory.appendingPathComponent(entry))
        }
        do {
            let candidate = try await LibraryRepository.open(.local(url: destination))
            try await candidate.importLibrary(source)
        }
        try fault?(.imported)
        let reopened = try await LibraryRepository.open(.local(url: destination))
        let actual = reopened.snapshot
        guard source == actual, try KudzuJSON.encoder(pretty: false).encode(source) == KudzuJSON.encoder(pretty: false).encode(actual) else {
            throw ServiceError("Migration verification failed; source and recovery copy retained")
        }
        try fault?(.verified)
        try fault?(.activating)
        if let sourceBytes {
            guard try Data(contentsOf: sourceURL) == sourceBytes else {
                throw ServiceError("The source library changed during migration. Originals retained; stop the old writer and retry.")
            }
        }
        let receipt = Receipt(role: role, recovery: recovery.lastPathComponent, store: "Library.store")
        try AtomicFile.write(try JSONEncoder().encode(receipt), to: marker, mode: 0o600)
        let directoryFD = Darwin.open(directory.path, O_RDONLY | O_CLOEXEC)
        if directoryFD >= 0 { _ = fsync(directoryFD); close(directoryFD) }
        return (reopened, firstRun)
    }

    /// lstat inspects the link itself. Never chmod or hash a copied symlink through its target.
    private static func entryType(_ url: URL) throws -> mode_t {
        var info = stat()
        guard lstat(url.path, &info) == 0 else {
            throw ServiceError("Cannot inspect migration entry \(url.lastPathComponent): \(String(cString: strerror(errno)))")
        }
        return info.st_mode & S_IFMT
    }

    private static func copyRecoveryEntry(from: URL, to: URL, relative: String,
                                          onlyMissing: Bool, omitted: inout [String]) throws {
        let fm = FileManager.default
        let kind = try entryType(from)
        if onlyMissing, fm.fileExists(atPath: to.path), kind != S_IFDIR { return }
        switch kind {
        case S_IFREG:
            try fm.copyItem(at: from, to: to)
            guard try entryType(to) == S_IFREG else { throw ServiceError("Recovery entry changed type: \(relative)") }
            try fm.setAttributes([.posixPermissions: 0o600], ofItemAtPath: to.path)
            guard try Data(contentsOf: from) == Data(contentsOf: to) else { throw ServiceError("Recovery copy differs: \(relative)") }
        case S_IFDIR:
            if !fm.fileExists(atPath: to.path) {
                try fm.createDirectory(at: to, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
            } else if try entryType(to) != S_IFDIR {
                throw ServiceError("Cannot merge a legacy directory over an existing file: \(relative)")
            }
            for child in try fm.contentsOfDirectory(atPath: from.path).sorted() {
                try copyRecoveryEntry(from: from.appendingPathComponent(child), to: to.appendingPathComponent(child),
                                      relative: relative + "/" + child, onlyMissing: onlyMissing, omitted: &omitted)
            }
        case S_IFSOCK where relative == AgentServer.socketName:
            omitted.append(relative)
        case S_IFLNK:
            throw ServiceError("Migration cannot make a self-contained recovery copy of symbolic link \(relative). Its target is unchanged; replace the link with a regular file before retrying.")
        default:
            throw ServiceError("Unsupported migration entry \(relative); originals retained")
        }
    }

    private static func validateInitializedStore(_ url: URL) throws {
        guard try entryType(url) == S_IFREG else { throw ServiceError("Active library is not a regular store file; originals retained") }
        let file = try FileHandle(forReadingFrom: url)
        defer { try? file.close() }
        guard try file.read(upToCount: 16) == Data("SQLite format 3\0".utf8) else {
            throw ServiceError("Active library is damaged or uninitialized. Restore a verified store backup before retrying; originals retained.")
        }
        let metadata = try NSPersistentStoreCoordinator.metadataForPersistentStore(ofType: NSSQLiteStoreType, at: url,
                                                                                   options: [NSReadOnlyPersistentStoreOption: true])
        guard let models = metadata[NSStoreModelVersionHashesKey] as? [String: Data],
              Set(["PersistedSnippet", "PersistedGroup", "PersistedUsage"]).isSubset(of: Set(models.keys)) else {
            throw ServiceError("Active library is not an initialized Kudzu store; restore a verified backup before retrying")
        }
    }
}
