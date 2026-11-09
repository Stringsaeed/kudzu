// Ported from kudzu-core/src/clipboard.rs
// Clipboard history: what a copy *is*, when it happened, and how long it's kept.
//
// Pure logic only. The engine reads the pasteboard as raw `(type, bytes)` pairs per item; this
// module classifies them ([`classify`]), folds repeats of the same content into one entry with
// its copy timings ([`History::ingest`]), applies the retention window ([`History::prune`]) and
// encodes the raw data for exact restoration ([`encode_items`] / [`decode_items`]).
//
// Entries hold metadata and a bounded text excerpt; full data lives in a blob next to the index.

import Foundation

extension Clipboard {
    public enum Ingest: Equatable, Sendable {
        case added(ClipEntry), repeated(ClipEntry)
        public var entry: ClipEntry { switch self { case .added(let e), .repeated(let e): e } }
    }
    /// The clipboard history, newest copy first. Repeats retain identity and corrections.
    public struct History: Codable, Equatable, Sendable {
        public var version: UInt32 = 1
        public var secretRules: UInt32 = SECRET_RULES
        public var entries: [ClipEntry] = []
        public init() {}
        enum CodingKeys: String, CodingKey { case version, secretRules, entries }
        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy:CodingKeys.self)
            version = try c.decodeIfPresent(UInt32.self,forKey:.version) ?? 1
            secretRules = try c.decodeIfPresent(UInt32.self,forKey:.secretRules) ?? 0
            entries = try c.decode([ClipEntry].self,forKey:.entries)
        }
        /// Rescans older excerpts once, preserving full-text detections beyond the excerpt.
        @discardableResult public mutating func rescanSecrets() -> Bool {
            guard secretRules < SECRET_RULES else { return false }
            for i in entries.indices {
                let found = secretOf(entries[i].kind,entries[i].text)
                if found != nil || entries[i].textLen <= TEXT_EXCERPT_LIMIT { entries[i].secret = found }
            }
            secretRules = SECRET_RULES; return true
        }
        /// Records the user's correction (`None` goes back to detection).
        @discardableResult public mutating func setSecret(_ id: UUID, secret: Bool?) -> ClipEntry? {
            guard let i = entries.firstIndex(where: { $0.id == id }) else { return nil }
            entries[i].secretOverride = secret; return entries[i]
        }
        public func get(_ id: UUID) -> ClipEntry? { entries.first { $0.id == id } }
        /// Records a copy. Repeated content keeps its id, pin and copy timings.
        @discardableResult public mutating func ingest(_ c: Classified, source: ClipSource? = nil, image: ImageSize? = nil, now: Date) -> Ingest {
            let hash = String(format:"%016llx",c.hash), pos = entries.firstIndex { $0.hash == hash }
            var e: ClipEntry
            if let pos { e = entries.remove(at:pos) }
            else { e = ClipEntry(kind:c.kind,hash:hash,firstCopiedAt:now,copiedAt:now,copyCount:0) }
            e.kind = c.kind; e.text = c.text.map { excerpt($0,limit:TEXT_EXCERPT_LIMIT) }
            e.textLen = c.text?.unicodeScalars.count ?? 0
            e.lineCount = c.text.map { $0.isEmpty ? 0 : max(1, RustText.lines($0).count) } ?? 0
            e.files = c.files; e.color = c.color; e.rich = c.rich; e.types = c.types; e.size = c.size
            if let image { e.image = image }
            e.secret = secretOf(c.kind,c.text); e.copiedAt = now
            if e.copyCount < UInt32.max { e.copyCount += 1 }
            e.timings.append(now); e.timings = Array(e.timings.suffix(MAX_TIMINGS))
            if let source { e.source = source }
            entries.insert(e,at:0)
            return pos == nil ? .added(e) : .repeated(e)
        }
        /// Removes expired and excess unpinned entries, returning them in history order.
        @discardableResult public mutating func prune(now: Date, retentionDays: UInt32) -> [ClipEntry] {
            let cutoff = now.addingTimeInterval(-Double(retentionDays)*86400)
            var unpinned = 0
            return extract { e in
                if e.pinned { return false }
                if (retentionDays > 0 && e.copiedAt < cutoff) || unpinned >= MAX_ENTRIES { return true }
                unpinned += 1; return false
            }
        }
        mutating func extract(_ predicate: (ClipEntry) -> Bool) -> [ClipEntry] {
            var removed: [ClipEntry] = [], kept: [ClipEntry] = []
            for e in entries { if predicate(e) { removed.append(e) } else { kept.append(e) } }
            entries = kept; return removed
        }
        /// Removes entries by id, returning them (in history order).
        @discardableResult public mutating func remove(_ ids: [UUID]) -> [ClipEntry] { extract { ids.contains($0.id) } }
        /// Undo: skips identities or hashes already present and restores chronological order.
        @discardableResult public mutating func restore(_ removed: [ClipEntry]) -> [ClipEntry] {
            var restored: [ClipEntry] = []
            for e in removed where !entries.contains(where: { $0.id == e.id || $0.hash == e.hash }) { restored.append(e); entries.append(e) }
            // `sort_by_key(Reverse(copied_at))` is stable: equal times keep their order.
            entries = entries.enumerated().sorted { $0.element.copiedAt != $1.element.copiedAt ? $0.element.copiedAt > $1.element.copiedAt : $0.offset < $1.offset }.map(\.element)
            return restored
        }
        /// Removes everything (pinned too when `include_pinned`), returning what was removed.
        @discardableResult public mutating func clear(includePinned: Bool) -> [ClipEntry] { extract { includePinned || !$0.pinned } }
        @discardableResult public mutating func setPinned(_ id: UUID, pinned: Bool) -> ClipEntry? {
            guard let i = entries.firstIndex(where: { $0.id == id }) else { return nil }
            entries[i].pinned = pinned; return entries[i]
        }
        /// Notes that the user put this entry back on the clipboard; it moves to the top.
        @discardableResult public mutating func markUsed(_ id: UUID, now: Date) -> ClipEntry? {
            guard let i = entries.firstIndex(where: { $0.id == id }) else { return nil }
            var e = entries.remove(at:i); e.lastUsedAt = now
            if e.useCount < UInt32.max { e.useCount += 1 }
            e.copiedAt = now; entries.insert(e,at:0); return e
        }
    }
}
