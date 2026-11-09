import Foundation
import KudzuCore
import KudzuEngine

/// Clipboard history: a watcher that records copies and the commands the UI calls. Port of
/// `clipboard/mod.rs`; media access (`kzclip://` in Tauri) is ``media(kind:id:index:)``.
///
/// The pasteboard has no change notification, so (like every clipboard manager) a background
/// timer polls its change count — one integer read every 300 ms. Reading and storing a copy
/// only happens when the count moves, off the main thread.
public final class ClipboardHistory: @unchecked Sendable {
    // @unchecked: `store` is only touched while holding `lock`; the poll state (`last`, `tick`,
    // `timer`) only on `queue`.

    /// What the watcher reads from settings on every change.
    public struct Config: Sendable {
        public var enabled: Bool
        public var ignoredApps: [String]
        public var retentionDays: UInt32
        public init(enabled: Bool, ignoredApps: [String], retentionDays: UInt32) {
            self.enabled = enabled
            self.ignoredApps = ignoredApps
            self.retentionDays = retentionDays
        }
    }

    static let poll: DispatchTimeInterval = .milliseconds(300)
    /// Save a dirty index at most this often (in polls).
    static let saveEvery: UInt32 = 3
    /// Apply retention this often (in polls): every 10 minutes.
    static let pruneEvery: UInt32 = 2000
    /// Larger copies keep their metadata and text but no restorable blob.
    public static let maxBlobBytes: UInt64 = 64 * 1024 * 1024
    /// Longer side of stored thumbnails, in pixels (sharp at 2× in the list and picker).
    public static let thumbPx = 480
    /// Image files bigger than this aren't read for a thumbnail.
    static let maxFileThumbBytes: UInt64 = 40 * 1024 * 1024
    /// Copied files bigger than this aren't served as previews.
    static let maxFileBytes: UInt64 = 80 * 1024 * 1024

    private let lock = NSLock()
    private var store: ClipStore
    private let system: any ClipboardSystem
    private let config: @Sendable () -> Config
    private let emit: @Sendable (ClipEvent) -> Void
    private let queue = DispatchQueue(label: "sh.saeed.kudzu.clipboard", qos: .utility)
    private var timer: DispatchSourceTimer?
    private var last = 0
    private var tick: UInt32 = 0
    private let log = Logging.logger(category: "clipboard")

    /// Loads the history in `dir` (the data folder's `clipboard/`).
    public init(
        dir: URL,
        system: any ClipboardSystem = LiveClipboardSystem(),
        config: @escaping @Sendable () -> Config,
        emit: @escaping @Sendable (ClipEvent) -> Void
    ) {
        store = ClipStore.load(dir)
        self.system = system
        self.config = config
        self.emit = emit
    }

    private func locked<T>(_ body: (inout ClipStore) throws -> T) rethrows -> T {
        lock.lock()
        defer { lock.unlock() }
        return try body(&store)
    }

    // MARK: Watcher

    /// Starts the watcher and the retention timer.
    public func start() {
        queue.async { [self] in
            guard timer == nil else { return }
            prune()
            // What's on the clipboard at launch was copied earlier; don't count it as a new copy.
            last = system.changeCount()
            let t = DispatchSource.makeTimerSource(queue: queue)
            t.schedule(deadline: .now() + Self.poll, repeating: Self.poll, leeway: .milliseconds(30))
            t.setEventHandler { [weak self] in self?.pollOnce() }
            timer = t
            t.resume()
        }
    }

    /// Stops the watcher (tests).
    public func stop() {
        queue.sync {
            timer?.cancel()
            timer = nil
        }
    }

    private func pollOnce() {
        tick &+= 1
        if tick % Self.saveEvery == 0 { locked { $0.saveIfDirty() } }
        if tick % Self.pruneEvery == 0 { prune() }
        let count = system.changeCount()
        guard count != last else { return }
        last = count
        if system.isOwnChange(count) { return }
        let c = config()
        guard c.enabled else { return }
        let front = system.frontmostApp()
        if let front, c.ignoredApps.contains(front.bundleId) { return }
        capture(front: front)
    }

    /// Reads the pasteboard and records it (one poll that saw a new change).
    func capture(front: AppInfo?) {
        let items = system.read()
        guard !items.isEmpty, !Clipboard.isPrivate(items) else { return }
        guard let c = Clipboard.classify(items, sourceBundle: front?.bundleId) else { return }
        let source = front.map { ClipSource(bundleId: $0.bundleId, name: $0.name, path: $0.path) }

        // Thumbnails: copied images, or a single copied image file.
        var thumb: EngineClipboard.Thumbnail?
        switch c.kind {
        case .image:
            if let (data, _) = Clipboard.imageOf(items) { thumb = system.thumbnail(data, maxPx: Self.thumbPx) }
        case .file where c.files.count == 1 && Clipboard.imageMimeForPath(c.files[0]) != nil:
            let path = c.files[0]
            if let attrs = try? FileManager.default.attributesOfItem(atPath: path),
               attrs[.type] as? FileAttributeType == .typeRegular,
               (attrs[.size] as? UInt64 ?? .max) <= Self.maxFileThumbBytes,
               let data = FileManager.default.contents(atPath: path) {
                thumb = system.thumbnail(data, maxPx: Self.thumbPx)
            }
        default: break
        }
        let image = thumb.map { ImageSize(width: UInt32(clamping: $0.width), height: UInt32(clamping: $0.height)) }

        let (ingest, blob, thumbURL) = locked { s in
            let ingest = s.history.ingest(c, source: source, image: image, now: Date())
            s.markDirty()
            return (ingest, s.blobURL(ingest.entry.id), s.thumbURL(ingest.entry.id))
        }
        // Same image bytes as before: the blob and thumbnail on disk are already right.
        var repeated = false
        if case .repeated = ingest { repeated = true }
        let unchanged = repeated && c.kind == .image && FileManager.default.fileExists(atPath: blob.path)
        if !unchanged {
            if c.size <= Self.maxBlobBytes {
                do {
                    try AtomicFile.write(Clipboard.encodeItems(items), to: blob, mode: 0o600)
                } catch {
                    log.warning("clipboard blob: \(error.localizedDescription)")
                }
            } else {
                try? FileManager.default.removeItem(at: blob)
            }
            if let thumb { try? thumb.png.write(to: thumbURL) }
        }
        emit(.upsert(entry: ingest.entry))
    }

    /// Saves the index now (on quit).
    public func flush() { locked { $0.saveIfDirty() } }

    /// Applies the retention window and size cap.
    public func prune() {
        let days = config().retentionDays
        let removed = locked { s -> [ClipEntry] in
            let removed = s.history.prune(now: Date(), retentionDays: days)
            if !removed.isEmpty {
                s.purge(removed)
                s.markDirty()
            }
            return removed
        }
        guard !removed.isEmpty else { return }
        log.info("clipboard: \(removed.count) entries expired")
        emit(.remove(ids: removed.map(\.id)))
    }

    /// `prune` on the watcher's queue (retention changed in Settings).
    public func pruneSoon() { queue.async { [self] in prune() } }

    // MARK: Reading

    public var entries: [ClipEntry] { locked { $0.history.entries } }

    public func entry(_ id: UUID) -> ClipEntry? { locked { $0.history.get(id) } }

    /// `clipboard_list`.
    public func list() -> ClipboardInfo {
        let (entries, usage) = locked { ($0.history.entries, $0.diskUsage()) }
        return ClipboardInfo(entries: entries, diskUsage: usage, canPaste: system.canPaste())
    }

    /// The full text of an entry (list payloads carry an excerpt).
    public func text(_ id: UUID) -> String? {
        locked { s in s.items(id).flatMap(Clipboard.textOf) ?? s.history.get(id)?.text }
    }

    /// The stored blob's text only (the agent's `clipboard_text`).
    public func storedText(_ id: UUID) -> String? {
        locked { $0.items(id).flatMap(Clipboard.textOf) }
    }

    /// Text that can become a snippet (`clipboard_to_snippet`).
    public func snippetText(_ id: UUID) throws -> String {
        try locked { s in
            guard let entry = s.history.get(id) else { throw ServiceError("Not found") }
            guard let text = s.items(id).flatMap(Clipboard.textOf) ?? entry.text else {
                throw ServiceError("Only text can become a snippet.")
            }
            return text
        }
    }

    // MARK: Commands

    /// Items to put back on the clipboard for an entry (plain text only when `plain`).
    func itemsFor(_ id: UUID, plain: Bool) throws -> [Clipboard.PasteItem] {
        try locked { s in
            guard let entry = s.history.get(id) else { throw ServiceError("That item is no longer in your clipboard history.") }
            let stored = s.items(id)
            let fullText = stored.flatMap(Clipboard.textOf) ?? (entry.textLen <= Clipboard.TEXT_EXCERPT_LIMIT ? entry.text : nil)
            if plain, let text = fullText {
                return [[(Clipboard.uti.PLAIN_TEXT, Data(text.utf8))]]
            }
            if let stored { return stored }
            if let text = fullText { return [[(Clipboard.uti.PLAIN_TEXT, Data(text.utf8))]] }
            throw ServiceError("This item was too large to keep, so it can't be copied again.")
        }
    }

    /// Puts an entry back on the clipboard (`clipboard_copy`).
    public func copy(_ id: UUID, plain: Bool) throws {
        let items = try itemsFor(id, plain: plain)
        system.write(items)
        let entry = locked { s -> ClipEntry? in
            let e = s.history.markUsed(id, now: Date())
            s.markDirty()
            return e
        }
        if let entry { emit(.upsert(entry: entry)) }
    }

    /// Whether ⌘V into other apps is possible.
    public func canPaste() -> Bool { system.canPaste() }

    /// Presses ⌘V into `target`. Blocks up to ~1 s: call off the main thread.
    public func paste(into target: AppInfo?) throws(EngineClipboard.PasteError) {
        try system.paste(target: target)
    }

    public func delete(_ ids: [UUID]) -> [ClipEntry] {
        let removed = locked { s -> [ClipEntry] in
            let removed = s.history.remove(ids)
            s.trash(removed)
            s.markDirty()
            return removed
        }
        emit(.remove(ids: removed.map(\.id)))
        return removed
    }

    /// Undo for delete (while the files are still in the trash).
    public func restore(_ entries: [ClipEntry]) {
        let all = locked { s -> [ClipEntry] in
            s.untrash(entries)
            s.history.restore(entries)
            s.markDirty()
            return s.history.entries
        }
        emit(.reset(entries: all))
    }

    public func pin(_ id: UUID, pinned: Bool) -> ClipEntry? {
        let entry = locked { s -> ClipEntry? in
            let e = s.history.setPinned(id, pinned: pinned)
            s.markDirty()
            return e
        }
        if let entry { emit(.upsert(entry: entry)) }
        return entry
    }

    /// Corrects secret detection for an entry: `false` shows it (to agents too), `true` hides
    /// it, `nil` goes back to what Kudzu detected.
    public func setSecret(_ id: UUID, secret: Bool?) -> ClipEntry? {
        let entry = locked { s -> ClipEntry? in
            let e = s.history.setSecret(id, secret: secret)
            s.markDirty()
            return e
        }
        if let entry { emit(.upsert(entry: entry)) }
        return entry
    }

    /// Deletes the whole history (pinned items too when asked). Not undoable.
    public func clear(includePinned: Bool) -> Int {
        let (n, all) = locked { s -> (Int, [ClipEntry]) in
            let removed = s.history.clear(includePinned: includePinned)
            s.purge(removed)
            s.emptyTrash() // not undoable: entries deleted earlier go for good too
            s.markDirty()
            s.saveIfDirty()
            return (removed.count, s.history.entries)
        }
        emit(.reset(entries: all))
        return n
    }

    // MARK: Media (replaces `kzclip://`)

    /// Clipboard media for the UI. Only entries in the history can be read, so this can't be
    /// used to read arbitrary files:
    ///
    /// - `thumb`: the stored PNG thumbnail.
    /// - `image`: the full copied image, in its original format.
    /// - `file`: the i-th copied file, only when it's an image (for previews).
    /// - `icon`: the Finder icon of the i-th copied file.
    public func media(kind: ClipMediaKind, id: UUID, index: Int) -> (data: Data, mime: String)? {
        let found = locked { s -> (ClipEntry, URL, [Clipboard.PasteItem]?)? in
            guard let entry = s.history.get(id) else { return nil }
            return (entry, s.thumbURL(id), kind == .image ? s.items(id) : nil)
        }
        guard let (entry, thumb, items) = found else { return nil }
        switch kind {
        case .thumb:
            return (try? Data(contentsOf: thumb)).map { ($0, "image/png") }
        case .image:
            guard let items, let (data, mime) = Clipboard.imageOf(items) else { return nil }
            return (data, mime)
        case .file:
            guard entry.files.indices.contains(index) else { return nil }
            let path = entry.files[index]
            guard let mime = Clipboard.imageMimeForPath(path),
                  let attrs = try? FileManager.default.attributesOfItem(atPath: path),
                  attrs[.type] as? FileAttributeType == .typeRegular,
                  (attrs[.size] as? UInt64 ?? .max) <= Self.maxFileBytes,
                  let data = FileManager.default.contents(atPath: path) else { return nil }
            return (data, mime)
        case .icon:
            guard entry.files.indices.contains(index) else { return nil }
            return system.iconPNG(path: entry.files[index], size: 64).map { ($0, "image/png") }
        }
    }
}
