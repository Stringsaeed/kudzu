import Foundation

// Types and constants of `kudzu-core/src/clipboard.rs` (clipboard history: what a copy *is*, when
// it happened, and how long it's kept). Classification, `History` and the pasteboard codec live in
// `Clipboard/`.

/// What kind of thing was copied. Decides the icon, preview and filters.
public enum ClipKind: String, Codable, Hashable, Sendable, CaseIterable {
    case text, code, link, color, image, file, other
}

/// What a secret-looking text resembles. Port of `secrets::SecretKind`.
public enum SecretKind: String, Codable, Hashable, Sendable, CaseIterable {
    /// A PEM or OpenSSH private key block.
    case privateKey
    /// A vendor API key or a long random-looking token.
    case apiKey
    /// A JWT or an `Authorization` header value.
    case token
    /// A labelled password, or a lone word that looks like one.
    case password
    /// A labelled secret, or a password inside a URL.
    case credential

    /// A short noun for messages ("withheld: API key").
    public var label: String {
        switch self {
        case .privateKey: "private key"
        case .apiKey: "API key"
        case .token: "access token"
        case .password: "password"
        case .credential: "secret"
        }
    }
}

/// The app that was frontmost when something was copied.
public struct ClipSource: Codable, Hashable, Sendable {
    public var bundleId: String
    public var name: String
    public var path: String?

    public init(bundleId: String, name: String, path: String? = nil) {
        self.bundleId = bundleId
        self.name = name
        self.path = path
    }
}

/// Pixel size of a copied image.
public struct ImageSize: Codable, Hashable, Sendable {
    public var width: UInt32
    public var height: UInt32

    public init(width: UInt32, height: UInt32) {
        self.width = width
        self.height = height
    }
}

/// One remembered copy. Repeats of the same content update this entry instead of adding rows.
public struct ClipEntry: Codable, Hashable, Sendable, Identifiable {
    // MARK: Constants

    /// Characters of text kept inline in an entry (enough for list rows and search).
    public static let textExcerptLimit = 2048
    /// Copy timestamps remembered per entry (the count keeps going beyond this).
    public static let maxTimings = 64
    /// Unpinned entries kept at most, whatever the retention window.
    public static let maxEntries = 5000
    /// Retention used when the setting is missing.
    public static let defaultRetentionDays: UInt32 = 7
    /// Bundle ids of apps whose copies Kudzu never records by default (password managers).
    public static let defaultIgnoredApps = [
        "com.1password.1password",
        "com.agilebits.onepassword7",
        "com.agilebits.onepassword-osx",
        "com.bitwarden.desktop",
        "com.apple.keychainaccess",
        "com.apple.Passwords",
        "org.keepassxc.keepassxc",
        "com.dashlane.dashlanephonefinal",
        "com.lastpass.LastPass",
        "in.sinew.Enpass-Desktop",
        "com.nordsec.nordpass",
        "me.proton.pass.electron",
    ]

    // MARK: Fields

    public var id: UUID
    public var kind: ClipKind
    /// Plain text excerpt (≤ ``textExcerptLimit`` characters); the full text lives in the blob.
    public var text: String?
    /// Full text length in characters.
    public var textLen: Int
    /// Lines in the full text.
    public var lineCount: Int
    /// File paths, for `file` entries.
    public var files: [String]
    public var image: ImageSize?
    /// Normalized CSS colour, for `color` entries.
    public var color: String?
    /// Whether formatted text (RTF/HTML) came with it.
    public var rich: Bool
    /// Pasteboard types, in the source app's order.
    public var types: [String]
    /// Bytes of all data.
    public var size: UInt64
    public var source: ClipSource?
    /// Content hash (FNV-1a, hex) used to fold repeats.
    public var hash: String
    public var firstCopiedAt: Date
    /// Most recent copy (or use); orders the history.
    public var copiedAt: Date
    public var copyCount: UInt32
    /// Most recent copy times, oldest first (≤ ``maxTimings``).
    public var timings: [Date]
    public var lastUsedAt: Date?
    public var useCount: UInt32
    /// Pinned entries survive retention and "Clear".
    public var pinned: Bool
    /// What the text looks like when it holds a password, key or token.
    public var secret: SecretKind?
    /// The user's correction: `false` = "not a secret", `true` = "hide it".
    public var secretOverride: Bool?

    public init(
        id: UUID = UUID(),
        kind: ClipKind,
        text: String? = nil,
        textLen: Int = 0,
        lineCount: Int = 0,
        files: [String] = [],
        image: ImageSize? = nil,
        color: String? = nil,
        rich: Bool = false,
        types: [String] = [],
        size: UInt64 = 0,
        source: ClipSource? = nil,
        hash: String,
        firstCopiedAt: Date,
        copiedAt: Date,
        copyCount: UInt32 = 1,
        timings: [Date] = [],
        lastUsedAt: Date? = nil,
        useCount: UInt32 = 0,
        pinned: Bool = false,
        secret: SecretKind? = nil,
        secretOverride: Bool? = nil
    ) {
        self.id = id
        self.kind = kind
        self.text = text
        self.textLen = textLen
        self.lineCount = lineCount
        self.files = files
        self.image = image
        self.color = color
        self.rich = rich
        self.types = types
        self.size = size
        self.source = source
        self.hash = hash
        self.firstCopiedAt = firstCopiedAt
        self.copiedAt = copiedAt
        self.copyCount = copyCount
        self.timings = timings
        self.lastUsedAt = lastUsedAt
        self.useCount = useCount
        self.pinned = pinned
        self.secret = secret
        self.secretOverride = secretOverride
    }

    /// Whether to treat this entry as a secret: the user's choice, else detection.
    public var isSecret: Bool { secretOverride ?? (secret != nil) }

    /// When retention removes this entry (`nil` when pinned or kept forever).
    public func expiresAt(retentionDays: UInt32) -> Date? {
        pinned || retentionDays == 0 ? nil : copiedAt.addingTimeInterval(Double(retentionDays) * 86_400)
    }

    // MARK: Codable

    private enum CodingKeys: String, CodingKey {
        case id, kind, text, textLen, lineCount, files, image, color, rich, types, size, source, hash
        case firstCopiedAt, copiedAt, copyCount, timings, lastUsedAt, useCount, pinned, secret, secretOverride
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(UUID.self, forKey: .id)
        kind = try c.decode(ClipKind.self, forKey: .kind)
        text = try c.decodeIfPresent(String.self, forKey: .text)
        textLen = try c.decode(Int.self, forKey: .textLen, default: 0)
        lineCount = try c.decode(Int.self, forKey: .lineCount, default: 0)
        files = try c.decode([String].self, forKey: .files, default: [])
        image = try c.decodeIfPresent(ImageSize.self, forKey: .image)
        color = try c.decodeIfPresent(String.self, forKey: .color)
        rich = try c.decode(Bool.self, forKey: .rich, default: false)
        types = try c.decode([String].self, forKey: .types, default: [])
        size = try c.decode(UInt64.self, forKey: .size, default: 0)
        source = try c.decodeIfPresent(ClipSource.self, forKey: .source)
        hash = try c.decode(String.self, forKey: .hash)
        firstCopiedAt = try c.decodeDate(forKey: .firstCopiedAt)
        copiedAt = try c.decodeDate(forKey: .copiedAt)
        copyCount = try c.decode(UInt32.self, forKey: .copyCount)
        timings = try c.decodeDates(forKey: .timings)
        lastUsedAt = try c.decodeDateIfPresent(forKey: .lastUsedAt)
        useCount = try c.decode(UInt32.self, forKey: .useCount, default: 0)
        pinned = try c.decode(Bool.self, forKey: .pinned, default: false)
        secret = try c.decodeIfPresent(SecretKind.self, forKey: .secret)
        secretOverride = try c.decodeIfPresent(Bool.self, forKey: .secretOverride)
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encodeUUID(id, forKey: .id)
        try c.encode(kind, forKey: .kind)
        try c.encodeIfPresent(text, forKey: .text)
        try c.encode(textLen, forKey: .textLen)
        try c.encode(lineCount, forKey: .lineCount)
        if !files.isEmpty { try c.encode(files, forKey: .files) }
        try c.encodeIfPresent(image, forKey: .image)
        try c.encodeIfPresent(color, forKey: .color)
        try c.encode(rich, forKey: .rich)
        try c.encode(types, forKey: .types)
        try c.encode(size, forKey: .size)
        try c.encodeIfPresent(source, forKey: .source)
        try c.encode(hash, forKey: .hash)
        try c.encodeDate(firstCopiedAt, forKey: .firstCopiedAt)
        try c.encodeDate(copiedAt, forKey: .copiedAt)
        try c.encode(copyCount, forKey: .copyCount)
        try c.encodeDates(timings, forKey: .timings)
        try c.encodeDateIfPresent(lastUsedAt, forKey: .lastUsedAt)
        try c.encode(useCount, forKey: .useCount)
        try c.encode(pinned, forKey: .pinned)
        try c.encodeIfPresent(secret, forKey: .secret)
        try c.encodeIfPresent(secretOverride, forKey: .secretOverride)
    }
}
