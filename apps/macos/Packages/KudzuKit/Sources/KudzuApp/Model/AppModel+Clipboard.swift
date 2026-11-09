import Foundation
import KudzuCore

/// The clipboard history mirrored from the backend (the store of `lib/clipboard.ts`).
public struct ClipboardState: Hashable, Sendable {
    public var ready = false
    /// Newest copy first.
    public var entries: [ClipEntry] = []
    /// Bytes on disk.
    public var diskUsage: UInt64 = 0
    /// Whether ⌘V into other apps is possible.
    public var canPaste = true

    public init() {}

    /// Inserts or replaces `entry`, keeping newest-first order (O(n), n ≤ 5,000).
    static func upsert(_ list: [ClipEntry], _ entry: ClipEntry) -> [ClipEntry] {
        var rest = list.filter { $0.id != entry.id }
        var i = 0
        while i < rest.count, rest[i].copiedAt > entry.copiedAt { i += 1 }
        rest.insert(entry, at: i)
        return rest
    }
}

extension AppModel {
    /// Applies a `clipboard` event.
    public func applyClipEvent(_ event: ClipEvent) {
        switch event {
        case let .upsert(entry):
            clipboard.entries = ClipboardState.upsert(clipboard.entries, entry)
            clipTextCache.retain(clipboard.entries)
        case let .remove(ids):
            clipTextCache.remove(ids)
            let gone = Set(ids)
            clipboard.entries.removeAll { gone.contains($0.id) }
        case let .reset(entries):
            clipTextCache.retain(entries)
            clipboard.entries = entries.sorted { $0.copiedAt > $1.copiedAt }
        }
    }

    /// Reloads the history.
    public func refreshClipboard() async {
        guard let info = try? await backend.clipboardList() else { return }
        clipboard.ready = true
        clipboard.entries = info.entries
        clipTextCache.retain(info.entries)
        clipboard.diskUsage = info.diskUsage
        clipboard.canPaste = info.canPaste
    }

    /// Deletes entries; returns them for undo.
    @discardableResult
    public func removeClips(_ ids: [UUID]) async throws -> [ClipEntry] {
        let removed = try await backend.clipboardDelete(ids: ids)
        clipTextCache.remove(ids)
        let gone = Set(ids)
        clipboard.entries.removeAll { gone.contains($0.id) }
        return removed
    }

    public func restoreClips(_ entries: [ClipEntry]) async throws {
        clipboard.entries = entries.reduce(clipboard.entries, ClipboardState.upsert)
        try await backend.clipboardRestore(entries)
    }

    public func setClipPinned(_ id: UUID, pinned: Bool) async throws {
        updateClip(id) { $0.pinned = pinned }
        _ = try await backend.clipboardPin(id: id, pinned: pinned)
    }

    /// `false` = "not a secret", `true` = "hide it", `nil` = back to detection.
    public func setClipSecret(_ id: UUID, secret: Bool?) async throws {
        updateClip(id) { $0.secretOverride = secret }
        _ = try await backend.clipboardSetSecret(id: id, secret: secret)
    }

    /// Clears the history (pinned entries stay unless `includePinned`); returns how many went.
    @discardableResult
    public func clearClipboard(includePinned: Bool) async throws -> Int {
        let n = try await backend.clipboardClear(includePinned: includePinned)
        clipboard.entries = includePinned ? [] : clipboard.entries.filter(\.pinned)
        clipTextCache.retain(clipboard.entries)
        await refreshClipboard()
        return n
    }

    private func updateClip(_ id: UUID, _ change: (inout ClipEntry) -> Void) {
        guard let i = clipboard.entries.firstIndex(where: { $0.id == id }) else { return }
        change(&clipboard.entries[i])
    }
}
