import Foundation
import KudzuCore

// The clipboard view's actions on top of the store mirror in `AppModel+Clipboard.swift`: the parts of
// `clipboard-view.tsx`, `clipboard-picker.tsx` and `lib/clipboard.ts` (`loadFullText`) that talk to the
// backend. Feedback (toasts, flashes, announcements) stays in the views.

/// A bounded cache belonging to one model/data source. Removed entries are evicted by history
/// events so deleted clipboard contents do not remain in a process-global promise map.
@MainActor
final class ClipTextCache {
    private struct Value {
        var hash: String
        var task: Task<String?, Never>
    }
    private var texts: [UUID: Value] = [:]
    private var order: [UUID] = []

    func task(for entry: ClipEntry) -> Task<String?, Never>? {
        guard let value = texts[entry.id], value.hash == entry.hash else { return nil }
        return value.task
    }

    func insert(_ task: Task<String?, Never>, for entry: ClipEntry) {
        remove([entry.id])
        texts[entry.id] = Value(hash: entry.hash, task: task)
        order.append(entry.id)
        if order.count > 64 { remove([order[0]]) }
    }

    func remove(_ ids: [UUID]) {
        let ids = Set(ids)
        for id in ids { texts.removeValue(forKey: id)?.task.cancel() }
        order.removeAll { ids.contains($0) }
    }

    func retain(_ entries: [ClipEntry]) {
        let hashes = Dictionary(entries.map { ($0.id, $0.hash) }, uniquingKeysWith: { _, last in last })
        remove(texts.compactMap { hashes[$0.key] == $0.value.hash ? nil : $0.key })
    }

    func discardFailed(_ task: Task<String?, Never>, for entry: ClipEntry) {
        if texts[entry.id]?.task == task { remove([entry.id]) }
    }
}

extension AppModel {
    // MARK: Text

    /// The full text of an entry: the inline excerpt when it holds everything, else fetched once per
    /// id + hash (an entry's content never changes under its hash).
    public func clipFullText(_ entry: ClipEntry) async -> String {
        if ClipPreviewRules.excerptIsComplete(textLen: entry.textLen, excerpt: entry.text) { return entry.text ?? "" }
        if let task = clipTextCache.task(for: entry) { return await task.value ?? entry.text ?? "" }
        let fallback = entry.text ?? ""
        let task = Task { [backend] in
            try? await backend.clipboardText(id: entry.id)
        }
        clipTextCache.insert(task, for: entry)
        if let text = await task.value { return text }
        clipTextCache.discardFailed(task, for: entry)
        return fallback
    }

    /// The full text when it is already known (excerpt complete or fetched), without waiting.
    public func clipKnownText(_ entry: ClipEntry) -> String? {
        guard ClipPreviewRules.excerptIsComplete(textLen: entry.textLen, excerpt: entry.text) else { return nil }
        return entry.text
    }

    // MARK: Counts

    /// Entries per filter (`counts` in `clipboard-view.tsx`).
    public func clipCounts() -> [ClipFilter: Int] {
        var out: [ClipFilter: Int] = [:]
        for f in Clipboard.FILTERS {
            guard let filter = ClipFilter(rawValue: f.value) else { continue }
            out[filter] = clipboard.entries.reduce(0) { $0 + (Clipboard.inFilter($1, filter: f.value) ? 1 : 0) }
        }
        return out
    }

    // MARK: Actions

    /// Copies an entry; `plain` strips formatting. Throws the backend's message.
    public func copyClip(_ entry: ClipEntry, plain: Bool) async throws {
        try await backend.clipboardCopy(id: entry.id, plain: plain)
    }

    /// Pastes into the app the picker opened over (or only copies without Accessibility).
    public func pasteClip(_ entry: ClipEntry, plain: Bool) async throws -> PasteOutcome {
        try await backend.clipboardPaste(id: entry.id, plain: plain)
    }

    /// "Save as snippet" from the main window: a new snippet with the full text, shown in All.
    public func saveClipAsSnippet(_ entry: ClipEntry) async throws {
        guard Clipboard.hasText(entry) else { return }
        let content = await clipFullText(entry)
        try await newSnippet(SnippetDraft(content: content))
        setView(.all)
    }

    /// Opens a link or file (file `index` for multi-file entries).
    public func openClip(_ entry: ClipEntry, index: Int? = nil) async throws {
        guard entry.kind == .link || entry.kind == .file else { return }
        try await backend.clipboardOpen(id: entry.id, index: index)
    }

    /// Shows a file entry (or one of its files) in Finder.
    public func revealClip(_ entry: ClipEntry, index: Int? = nil) async throws {
        guard entry.kind == .file else { return }
        try await backend.clipboardReveal(id: entry.id, index: index)
    }

    /// Pins or unpins several entries: pins all unless every one is already pinned. Returns the new state.
    @discardableResult
    public func togglePinned(_ entries: [ClipEntry]) -> Bool? {
        guard !entries.isEmpty else { return nil }
        guard let pin = ClipSelectionRules.pinTarget(entries) else { return nil }
        for e in entries { Task { try? await setClipPinned(e.id, pinned: pin) } }
        return pin
    }

    /// Marks an entry as a secret or not. Matching Kudzu's own detection clears the correction
    /// instead of storing a redundant one. Returns the override before, for undo.
    @discardableResult
    public func markClipSecret(_ entry: ClipEntry, secret: Bool) -> Bool? {
        let before = entry.secretOverride
        let next = ClipSecretRules.override(for: entry, secret: secret)
        Task { try? await setClipSecret(entry.id, secret: next) }
        return before
    }
}

/// Avoid persisting a correction that agrees with automatic detection.
enum ClipSecretRules {
    static func override(for entry: ClipEntry, secret: Bool) -> Bool? {
        secret == (entry.secret != nil) ? nil : secret
    }
}

/// Bulk pinning unpins only when every selected entry is already pinned.
enum ClipSelectionRules {
    static func pinTarget(_ entries: [ClipEntry]) -> Bool? {
        guard !entries.isEmpty else { return nil }
        return !entries.allSatisfy(\.pinned)
    }
}
