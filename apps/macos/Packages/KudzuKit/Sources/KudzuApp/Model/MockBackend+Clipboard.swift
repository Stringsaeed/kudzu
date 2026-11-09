import Foundation
import KudzuCore

// Clipboard history commands of `lib/mock.ts`.

extension MockBackend {
    public func clipboardList() -> ClipboardInfo {
        ClipboardInfo(entries: clips, diskUsage: 38_400_000, canPaste: true)
    }

    public func clipboardText(id: UUID) -> String? {
        clips.first { $0.id == id }?.text
    }

    public func clipboardCopy(id: UUID, plain: Bool) {
        markUsed(id)
    }

    public func clipboardPaste(id: UUID, plain: Bool) -> PasteOutcome {
        markUsed(id)
        return .pasted
    }

    /// Moves the entry to the top as just used (the mock also wrote its text to the browser clipboard).
    private func markUsed(_ id: UUID) {
        guard var used = clips.first(where: { $0.id == id }) else { return }
        let now = Date()
        used.copiedAt = now
        used.lastUsedAt = now
        used.useCount += 1
        clips = [used] + clips.filter { $0.id != id }
        bus.send(.clipboard(.upsert(entry: used)))
    }

    public func clipboardDelete(ids: [UUID]) -> [ClipEntry] {
        let set = Set(ids)
        let gone = clips.filter { set.contains($0.id) }
        clips.removeAll { set.contains($0.id) }
        bus.send(.clipboard(.remove(ids: Array(set))))
        return gone
    }

    public func clipboardRestore(_ entries: [ClipEntry]) {
        clips = (clips + entries).sorted { $0.copiedAt > $1.copiedAt }
        bus.send(.clipboard(.reset(entries: clips)))
    }

    public func clipboardSetSecret(id: UUID, secret: Bool?) -> ClipEntry? {
        update(id) { $0.secretOverride = secret }
    }

    public func clipboardPin(id: UUID, pinned: Bool) -> ClipEntry? {
        update(id) { $0.pinned = pinned }
    }

    private func update(_ id: UUID, _ change: (inout ClipEntry) -> Void) -> ClipEntry? {
        guard let i = clips.firstIndex(where: { $0.id == id }) else { return nil }
        change(&clips[i])
        bus.send(.clipboard(.upsert(entry: clips[i])))
        return clips[i]
    }

    public func clipboardClear(includePinned: Bool) -> Int {
        let before = clips.count
        clips = includePinned ? [] : clips.filter(\.pinned)
        bus.send(.clipboard(.reset(entries: clips)))
        return before - clips.count
    }

    public func clipboardToSnippet(id: UUID) {
        let text = clips.first { $0.id == id }?.text ?? ""
        bus.send(.newSnippet(BackendEvent.NewSnippet(content: text)))
    }

    public func clipboardOpen(id: UUID, index: Int?) {}
    public func clipboardReveal(id: UUID, index: Int?) {}
    public func showClipboard() {}
    public func hideClipboard() {}
    public func showMainView(_ view: Route) {}

    public func setClipboardHotkey(accelerator: String) -> Settings {
        state.settings.clipboardHotkey = accelerator
        commitSettings()
        return state.settings
    }

    /// Thumbnails and full images of the sample image entries (`thumb` and `image` share one PNG,
    /// as the mock's URLs did); files and icons have none.
    public func clipboardMedia(kind: ClipMediaKind, id: UUID, index: Int) -> Data? {
        guard kind == .thumb || kind == .image, let art = artwork[id] else { return nil }
        if let cached = media[id] { return cached }
        let png = MockMedia.png(art)
        media[id] = png
        return png
    }
}
