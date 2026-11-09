import Foundation

/// What changed in the clipboard history, for every window. Port of `src-tauri/src/clipboard`'s
/// `ClipEvent`; JSON is tagged by `type`.
public enum ClipEvent: Codable, Hashable, Sendable {
    /// Added or changed; newest copies go to the top.
    case upsert(entry: ClipEntry)
    case remove(ids: [UUID])
    /// Many changes at once (clear, restore, retention).
    case reset(entries: [ClipEntry])

    private enum CodingKeys: String, CodingKey { case type, entry, ids, entries }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .type) {
        case "upsert": self = .upsert(entry: try c.decode(ClipEntry.self, forKey: .entry))
        case "remove": self = .remove(ids: try c.decode([UUID].self, forKey: .ids))
        case "reset": self = .reset(entries: try c.decode([ClipEntry].self, forKey: .entries))
        case let other: throw DecodingError.dataCorruptedError(forKey: .type, in: c, debugDescription: "unknown clip event \(other)")
        }
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .upsert(entry):
            try c.encode("upsert", forKey: .type)
            try c.encode(entry, forKey: .entry)
        case let .remove(ids):
            try c.encode("remove", forKey: .type)
            try c.encodeUUIDs(ids, forKey: .ids)
        case let .reset(entries):
            try c.encode("reset", forKey: .type)
            try c.encode(entries, forKey: .entries)
        }
    }
}

/// The clipboard history as `clipboard_list` returns it.
public struct ClipboardInfo: Codable, Hashable, Sendable {
    public var entries: [ClipEntry]
    /// Bytes on disk.
    public var diskUsage: UInt64
    /// Whether ⌘V into other apps is possible (Accessibility granted, no Secure Input).
    public var canPaste: Bool

    public init(entries: [ClipEntry], diskUsage: UInt64, canPaste: Bool) {
        self.entries = entries
        self.diskUsage = diskUsage
        self.canPaste = canPaste
    }
}

/// What `clipboard_paste` managed: pasted into the previous app, or only copied (no Accessibility).
public enum PasteOutcome: String, Codable, Hashable, Sendable {
    case pasted
    case copied
}

/// Clipboard media the UI shows (served by `kzclip://` in the Tauri app).
public enum ClipMediaKind: String, Codable, Hashable, Sendable {
    /// A ≤ 480 px PNG thumbnail of an image entry.
    case thumb
    /// The full image.
    case image
    /// A file of a `file` entry (by index).
    case file
    /// The Finder icon of a file (by index).
    case icon
}

/// A filter in the clipboard view (`lib/clipboard.ts` `ClipFilter`): everything, pinned, or one kind.
public enum ClipFilter: Codable, Hashable, Sendable {
    case all
    case pinned
    case kind(ClipKind)

    /// The string the TypeScript uses (`"all"`, `"pinned"` or a ``ClipKind`` raw value).
    public var rawValue: String {
        switch self {
        case .all: "all"
        case .pinned: "pinned"
        case let .kind(kind): kind.rawValue
        }
    }

    public init?(rawValue: String) {
        switch rawValue {
        case "all": self = .all
        case "pinned": self = .pinned
        default:
            guard let kind = ClipKind(rawValue: rawValue) else { return nil }
            self = .kind(kind)
        }
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.singleValueContainer()
        let raw = try c.decode(String.self)
        guard let value = ClipFilter(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(in: c, debugDescription: "unknown clipboard filter \(raw)")
        }
        self = value
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.singleValueContainer()
        try c.encode(rawValue)
    }
}
