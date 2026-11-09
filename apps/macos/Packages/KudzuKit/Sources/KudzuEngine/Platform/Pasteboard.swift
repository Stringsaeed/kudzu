import AppKit
import Foundation
import KudzuCore

// General pasteboard access with full save/restore, and paste-based insertion.
// Port of `macos/pasteboard.rs`.

/// Why a paste failed.
enum PasteFailure: Error, Hashable, Sendable {
    case unavailable
    case key(KeyError)
}

enum Pasteboard {
    /// There's no signal that the target consumed ⌘V; Electron apps can take well over 250 ms.
    static let restoreDelay: TimeInterval = 0.4

    /// Every item and type on the pasteboard, as raw data.
    struct Snapshot: Sendable {
        var items: [Clipboard.PasteItem]
    }

    private struct PendingRestore: Sendable {
        var snapshot: Snapshot
        /// Change count after our write.
        var count: Int
    }

    /// A paste whose old clipboard still has to come back.
    private static let pendingRestore = Locked<PendingRestore?>(nil)

    /// Change counts produced by Kudzu's own writes (restores, copies), newest last. The clipboard
    /// history skips these so putting the user's clipboard back never looks like a new copy.
    private static let ownChanges = Locked<[Int]>(Array(repeating: -1, count: 8))

    private static func noteOwnChange() {
        let count = changeCount()
        ownChanges.withLock { own in
            own.removeFirst()
            own.append(count)
        }
    }

    /// Whether `count` came from one of Kudzu's own writes.
    static func isOwnChange(_ count: Int) -> Bool {
        ownChanges.withLock { $0.contains(count) }
    }

    private static var general: NSPasteboard { NSPasteboard.general }

    /// Current change count of the general pasteboard.
    static func changeCount() -> Int {
        autoreleasepool { general.changeCount }
    }

    /// Saves every item; `nil` if some promised data can't be read (restoring would lose it).
    static func snapshot() -> Snapshot? {
        autoreleasepool {
            var items: [Clipboard.PasteItem] = []
            for item in general.pasteboardItems ?? [] {
                var entries: Clipboard.PasteItem = []
                for kind in item.types {
                    guard let data = item.data(forType: kind) else { return nil }
                    entries.append((kind.rawValue, data))
                }
                items.append(entries)
            }
            return Snapshot(items: items)
        }
    }

    /// Replaces the pasteboard with `snapshot`.
    static func restore(_ snapshot: Snapshot) {
        writeRaw(snapshot.items)
        noteOwnChange()
    }

    /// Every item and type, skipping types whose (promised) data can't be read.
    static func readItems() -> [Clipboard.PasteItem] {
        autoreleasepool {
            var items: [Clipboard.PasteItem] = []
            for item in general.pasteboardItems ?? [] {
                let entries: Clipboard.PasteItem = item.types.compactMap { kind in
                    item.data(forType: kind).map { (kind.rawValue, $0) }
                }
                if !entries.isEmpty {
                    items.append(entries)
                }
            }
            return items
        }
    }

    /// Replaces the pasteboard with `items` (as Kudzu's own change). Returns the new change count.
    static func writeItems(_ items: [Clipboard.PasteItem]) -> Int {
        writeRaw(items)
        noteOwnChange()
        return changeCount()
    }

    private static func writeRaw(_ items: [Clipboard.PasteItem]) {
        autoreleasepool {
            let board = general
            board.clearContents()
            guard !items.isEmpty else { return }
            let objects: [NSPasteboardItem] = items.map { entries in
                let item = NSPasteboardItem()
                for (kind, data) in entries {
                    item.setData(data, forType: NSPasteboard.PasteboardType(kind))
                }
                return item
            }
            board.writeObjects(objects)
        }
    }

    /// Plain-text contents.
    static func readString() -> String? {
        autoreleasepool { general.string(forType: .string) }
    }

    /// Writes plain text as the only item.
    @discardableResult
    static func writeString(_ text: String) -> Bool {
        let ok = autoreleasepool {
            let board = general
            board.clearContents()
            return board.setString(text, forType: .string)
        }
        noteOwnChange()
        return ok
    }

    /// Writes `text` marked transient and concealed so clipboard managers skip it.
    private static func writeTransient(_ text: String, html: String?) -> Bool {
        autoreleasepool {
            let item = NSPasteboardItem()
            let empty = Data()
            let ok = item.setString(text, forType: .string)
                && item.setData(empty, forType: NSPasteboard.PasteboardType("org.nspasteboard.TransientType"))
                && item.setData(empty, forType: NSPasteboard.PasteboardType("org.nspasteboard.ConcealedType"))
            if !ok {
                return false
            }
            if let html, !item.setString(html, forType: NSPasteboard.PasteboardType("public.html")) {
                return false
            }
            let board = general
            board.clearContents()
            return board.writeObjects([item])
        }
    }

    /// Pastes `text` via ⌘V, then restores the previous clipboard after a delay (blocking).
    static func pasteText(_ text: String, html: String?) throws(PasteFailure) {
        // A previous paste still pending restoration owns the "real" clipboard; put it back first.
        restorePending()
        guard let saved = snapshot() else { throw .unavailable }
        if !writeTransient(text, html: html) {
            restore(saved)
            throw .unavailable
        }
        let count = changeCount()
        pendingRestore.withLock { $0 = PendingRestore(snapshot: saved, count: count) }
        do {
            try Keys.paste()
        } catch {
            restorePending()
            throw .key(error)
        }
        Thread.sleep(forTimeInterval: restoreDelay)
        restorePending()
    }

    /// Restores the clipboard a paste replaced, unless the user copied something since.
    static func restorePending() {
        guard let pending = pendingRestore.withLock({ value -> PendingRestore? in
            defer { value = nil }
            return value
        }) else { return }
        if changeCount() == pending.count {
            restore(pending.snapshot)
        }
    }

    /// Copies the frontmost app's selection with ⌘C, waiting up to `timeout`; restores the clipboard.
    static func copySelection(timeout: TimeInterval) -> String? {
        restorePending()
        guard let saved = snapshot() else { return nil }
        let original = changeCount()
        do { try Keys.copy() } catch { return nil }
        let deadline = RuntimeTiming.now() + timeout
        var text: String?
        while RuntimeTiming.now() < deadline {
            Thread.sleep(forTimeInterval: 0.01)
            if changeCount() != original {
                text = readString().flatMap { $0.isEmpty ? nil : $0 }
                break
            }
        }
        if changeCount() != original {
            restore(saved)
        }
        return text
    }
}
