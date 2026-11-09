import Foundation
import KudzuCore

/// Clipboard history on disk. Port of `clipboard/store.rs`; the same layout, so a history the
/// Tauri app recorded loads unchanged:
///
/// ```text
/// clipboard/            (0700: only this user can read it)
///   history.json        index: metadata and short text excerpts (Clipboard.History)
///   items/<id>.kzc      every pasteboard type, for exact restoration
///   thumbs/<id>.png     image thumbnails
///   trash/              deleted entries' files, kept until the next launch (or Clear history) for Undo
/// ```
///
/// The index is written atomically and debounced; blobs are written once per copy. Not
/// thread-safe on its own: ``ClipboardHistory`` guards it with a lock.
public struct ClipStore {
    public let dir: URL
    public var history: Clipboard.History
    public private(set) var dirty: Bool

    private static let log = Logging.logger(category: "clipboard")

    /// Loads (or creates) the history in `dir`. An index that can't be parsed is moved aside
    /// (`history-corrupt-*.json`); one that can't be read starts a fresh history, as in Rust.
    public static func load(_ dir: URL) -> ClipStore {
        let fm = FileManager.default
        for sub in ["items", "thumbs", "trash"] {
            try? fm.createDirectory(at: dir.appendingPathComponent(sub, isDirectory: true), withIntermediateDirectories: true)
        }
        restrict(dir)
        let index = dir.appendingPathComponent("history.json")
        var history = Clipboard.History()
        // Whether blobs on disk may belong to an index this launch couldn't use.
        var unmapped = false
        if fm.fileExists(atPath: index.path) {
            switch Result(catching: { try Data(contentsOf: index) }) {
            case let .success(bytes):
                do {
                    history = try KudzuJSON.decode(Clipboard.History.self, from: bytes)
                } catch {
                    log.error("clipboard history unreadable (\(error.localizedDescription)); starting fresh")
                    let aside = dir.appendingPathComponent("history-corrupt-\(UUID().lowercased).json")
                    do {
                        try fm.moveItem(at: index, to: aside)
                    } catch {
                        log.error("couldn't move the unreadable clipboard history aside: \(error.localizedDescription)")
                        unmapped = true
                    }
                }
            case let .failure(error):
                log.error("clipboard history couldn't be read (\(error.localizedDescription)); starting fresh")
                unmapped = true
            }
        }
        // Entries from before secret detection (or older rules) are scanned once.
        let dirty = history.rescanSecrets()
        let store = ClipStore(dir: dir, history: history, dirty: dirty)
        // The trash only ever holds entries the user deleted: always empty it, like Rust.
        store.emptyTrash()
        // A quarantined index may be the only map back to the blobs. Keep unknown blobs across
        // launches until the user has recovered it; deleting them would make recovery impossible.
        let hasRecovery = ((try? fm.contentsOfDirectory(atPath: dir.path)) ?? []).contains { $0.hasPrefix("history-corrupt-") }
        if !unmapped && !hasRecovery {
            store.removeOrphans()
        }
        return store
    }

    public func blobURL(_ id: UUID) -> URL { dir.appendingPathComponent("items/\(id.lowercased).kzc") }
    public func thumbURL(_ id: UUID) -> URL { dir.appendingPathComponent("thumbs/\(id.lowercased).png") }
    func trashURL(_ file: URL) -> URL { dir.appendingPathComponent("trash/\(file.lastPathComponent)") }

    public mutating func markDirty() { dirty = true }

    /// Writes the index if anything changed since the last save.
    public mutating func saveIfDirty() {
        guard dirty else { return }
        dirty = false
        do {
            let json = try KudzuJSON.encode(history, pretty: false)
            try AtomicFile.write(json, to: dir.appendingPathComponent("history.json"), mode: 0o600)
        } catch {
            Self.log.error("saving clipboard history failed: \(error.localizedDescription)")
            dirty = true
        }
    }

    /// Raw pasteboard items of an entry.
    public func items(_ id: UUID) -> [Clipboard.PasteItem]? {
        guard let data = try? Data(contentsOf: blobURL(id)) else { return nil }
        return Clipboard.decodeItems(data)
    }

    /// Moves entries' files to the trash (Undo can bring them back until the next launch).
    public func trash(_ entries: [ClipEntry]) {
        for e in entries {
            for file in [blobURL(e.id), thumbURL(e.id)] {
                _ = rename(file.path, trashURL(file).path)
            }
        }
    }

    public func untrash(_ entries: [ClipEntry]) {
        for e in entries {
            for file in [blobURL(e.id), thumbURL(e.id)] {
                _ = rename(trashURL(file).path, file.path)
            }
        }
    }

    /// Deletes entries' files for good (expiry, Clear history).
    public func purge(_ entries: [ClipEntry]) {
        let fm = FileManager.default
        for e in entries {
            try? fm.removeItem(at: blobURL(e.id))
            try? fm.removeItem(at: thumbURL(e.id))
        }
    }

    /// Deletes every trashed file for good (launch, Clear history).
    func emptyTrash() {
        let fm = FileManager.default
        let trash = dir.appendingPathComponent("trash", isDirectory: true)
        for entry in (try? fm.contentsOfDirectory(at: trash, includingPropertiesForKeys: nil)) ?? [] {
            try? fm.removeItem(at: entry)
        }
    }

    /// Files whose entry is gone (a crash between delete and cleanup).
    private func removeOrphans() {
        let fm = FileManager.default
        let live = Set(history.entries.map { $0.id.lowercased })
        for sub in ["items", "thumbs"] {
            let folder = dir.appendingPathComponent(sub, isDirectory: true)
            for file in (try? fm.contentsOfDirectory(at: folder, includingPropertiesForKeys: nil)) ?? [] {
                let stem = file.deletingPathExtension().lastPathComponent.lowercased()
                if !live.contains(stem) { try? fm.removeItem(at: file) }
            }
        }
    }

    /// Bytes used on disk by blobs, thumbnails and deleted entries awaiting the trash.
    public func diskUsage() -> UInt64 {
        let fm = FileManager.default
        var total: UInt64 = 0
        for sub in ["items", "thumbs", "trash"] {
            let folder = dir.appendingPathComponent(sub, isDirectory: true)
            for file in (try? fm.contentsOfDirectory(at: folder, includingPropertiesForKeys: [.fileSizeKey])) ?? [] {
                total += UInt64((try? file.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0)
            }
        }
        return total
    }

    /// Clipboard history is private: owner-only permissions on the folder.
    private static func restrict(_ dir: URL) {
        try? FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: dir.path)
    }
}
