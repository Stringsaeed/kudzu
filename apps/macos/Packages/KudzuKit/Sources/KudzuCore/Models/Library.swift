import Foundation

/// Usage of one snippet on one device. Summed across devices for display.
public struct UsageRecord: Codable, Hashable, Sendable {
    public var snippetID: UUID
    public var deviceID: String
    public var count: Int
    public var charsSaved: Int
    public var lastUsedAt: Date?

    public init(snippetID: UUID, deviceID: String = "", count: Int = 0, charsSaved: Int = 0, lastUsedAt: Date? = nil) {
        self.snippetID = snippetID
        self.deviceID = deviceID
        self.count = count
        self.charsSaved = charsSaved
        self.lastUsedAt = lastUsedAt
    }

    private enum CodingKeys: String, CodingKey { case snippetID, deviceID, count, charsSaved, lastUsedAt }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        snippetID = try c.decode(UUID.self, forKey: .snippetID)
        deviceID = try c.decode(String.self, forKey: .deviceID, default: "")
        count = try c.decode(Int.self, forKey: .count, default: 0)
        charsSaved = try c.decode(Int.self, forKey: .charsSaved, default: 0)
        lastUsedAt = try c.decodeDateIfPresent(forKey: .lastUsedAt)
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encodeUUID(snippetID, forKey: .snippetID)
        try c.encode(deviceID, forKey: .deviceID)
        try c.encode(count, forKey: .count)
        try c.encode(charsSaved, forKey: .charsSaved)
        try c.encodeWholeSecondsIfPresent(lastUsedAt, forKey: .lastUsedAt)
    }
}

/// A whole library: the unit of import, export and backup. Port of `KudzuLibrary`.
public struct Library: Codable, Hashable, Sendable {
    /// The library format version this build writes.
    public static let currentFormatVersion = 1

    public var formatVersion: Int
    public var groups: [Group]
    public var snippets: [Snippet]
    public var usage: [UsageRecord]

    public init(formatVersion: Int = Library.currentFormatVersion, groups: [Group] = [], snippets: [Snippet] = [], usage: [UsageRecord] = []) {
        self.formatVersion = formatVersion
        self.groups = groups
        self.snippets = snippets
        self.usage = usage
    }

    /// An empty library at the current format version.
    public static let empty = Library()

    private enum CodingKeys: String, CodingKey { case formatVersion, groups, snippets, usage }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        formatVersion = try c.decode(Int.self, forKey: .formatVersion, default: Library.currentFormatVersion)
        groups = try c.decode([Group].self, forKey: .groups, default: [])
        snippets = try c.decode([Snippet].self, forKey: .snippets, default: [])
        usage = try c.decode([UsageRecord].self, forKey: .usage, default: [])
    }
}
