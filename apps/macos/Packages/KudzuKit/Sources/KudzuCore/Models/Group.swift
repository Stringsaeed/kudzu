import Foundation

/// A (nestable) folder of snippets that can carry an app rule and insertion override.
/// Port of `GroupRecord`.
///
/// In files that also import SwiftUI (which has its own `Group`), use ``SnippetGroup``.
public struct Group: Codable, Hashable, Sendable, Identifiable {
    public var id: UUID
    public var name: String
    public var parentID: UUID?
    public var sortIndex: Int
    public var isEnabled: Bool
    public var insertion: InsertionMethod?
    public var rule: AppRule?

    /// A new enabled root group (`GroupRecord::new`).
    public init(
        id: UUID = UUID(),
        name: String = "",
        parentID: UUID? = nil,
        sortIndex: Int = 0,
        isEnabled: Bool = true,
        insertion: InsertionMethod? = nil,
        rule: AppRule? = nil
    ) {
        self.id = id
        self.name = name
        self.parentID = parentID
        self.sortIndex = sortIndex
        self.isEnabled = isEnabled
        self.insertion = insertion
        self.rule = rule
    }

    // MARK: Codable

    private enum CodingKeys: String, CodingKey { case id, name, parentID, sortIndex, isEnabled, insertion, rule }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(UUID.self, forKey: .id)
        name = try c.decode(String.self, forKey: .name, default: "")
        parentID = try c.decodeIfPresent(UUID.self, forKey: .parentID)
        sortIndex = try c.decode(Int.self, forKey: .sortIndex, default: 0)
        isEnabled = try c.decode(Bool.self, forKey: .isEnabled, default: true)
        insertion = try c.decodeIfPresent(InsertionMethod.self, forKey: .insertion)
        rule = try c.decodeIfPresent(AppRule.self, forKey: .rule)
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encodeUUID(id, forKey: .id)
        try c.encode(name, forKey: .name)
        try c.encodeUUIDIfPresent(parentID, forKey: .parentID)
        try c.encode(sortIndex, forKey: .sortIndex)
        try c.encode(isEnabled, forKey: .isEnabled)
        try c.encodeIfPresent(insertion, forKey: .insertion)
        try c.encodeIfPresent(rule, forKey: .rule)
    }
}

/// ``Group`` under a name that doesn't collide with SwiftUI's `Group`.
public typealias SnippetGroup = Group
