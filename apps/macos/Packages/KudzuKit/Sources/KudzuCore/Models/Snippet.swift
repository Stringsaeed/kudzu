import Foundation

/// A snippet: one or more triggers expanding to a template. Port of `SnippetRecord`.
///
/// JSON matches `kudzu-core/src/types.rs`: `groupID`, whole-second dates, optional fields omitted
/// when `nil`, and serde's defaults for missing keys (`isEnabled` true, dates now).
public struct Snippet: Codable, Hashable, Sendable, Identifiable {
    public var id: UUID
    public var name: String
    /// One or more triggers. Never empty for an enabled snippet.
    public var triggers: [String]
    /// Template source; see docs/TEMPLATE_SYNTAX.md.
    public var content: String
    public var formatting: Formatting?
    public var groupID: UUID?
    public var fireMode: FireMode
    public var caseMode: CaseMode
    /// `nil` inherits from the group, then the global default.
    public var insertion: InsertionMethod?
    /// `nil` inherits the group's rule.
    public var rule: AppRule?
    public var isFavorite: Bool
    public var isEnabled: Bool
    public var createdAt: Date
    public var updatedAt: Date

    /// A snippet with default modes (`SnippetRecord::new` when only name/triggers/content are given).
    /// `updatedAt` defaults to `createdAt`.
    public init(
        id: UUID = UUID(),
        name: String = "",
        triggers: [String] = [],
        content: String = "",
        formatting: Formatting? = nil,
        groupID: UUID? = nil,
        fireMode: FireMode = .immediate,
        caseMode: CaseMode = .propagate,
        insertion: InsertionMethod? = nil,
        rule: AppRule? = nil,
        isFavorite: Bool = false,
        isEnabled: Bool = true,
        createdAt: Date = Date(),
        updatedAt: Date? = nil
    ) {
        self.id = id
        self.name = name
        self.triggers = triggers
        self.content = content
        self.formatting = formatting
        self.groupID = groupID
        self.fireMode = fireMode
        self.caseMode = caseMode
        self.insertion = insertion
        self.rule = rule
        self.isFavorite = isFavorite
        self.isEnabled = isEnabled
        self.createdAt = createdAt
        self.updatedAt = updatedAt ?? createdAt
    }

    // MARK: Codable

    private enum CodingKeys: String, CodingKey {
        case id, name, triggers, content, formatting, groupID, fireMode, caseMode, insertion, rule
        case isFavorite, isEnabled, createdAt, updatedAt
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(UUID.self, forKey: .id)
        name = try c.decode(String.self, forKey: .name, default: "")
        triggers = try c.decode([String].self, forKey: .triggers, default: [])
        content = try c.decode(String.self, forKey: .content, default: "")
        formatting = try c.decodeIfPresent(Formatting.self, forKey: .formatting)
        groupID = try c.decodeIfPresent(UUID.self, forKey: .groupID)
        fireMode = try c.decode(FireMode.self, forKey: .fireMode, default: .immediate)
        caseMode = try c.decode(CaseMode.self, forKey: .caseMode, default: .propagate)
        insertion = try c.decodeIfPresent(InsertionMethod.self, forKey: .insertion)
        rule = try c.decodeIfPresent(AppRule.self, forKey: .rule)
        isFavorite = try c.decode(Bool.self, forKey: .isFavorite, default: false)
        isEnabled = try c.decode(Bool.self, forKey: .isEnabled, default: true)
        createdAt = try c.decodeDateIfPresent(forKey: .createdAt) ?? Date()
        updatedAt = try c.decodeDateIfPresent(forKey: .updatedAt) ?? Date()
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encodeUUID(id, forKey: .id)
        try c.encode(name, forKey: .name)
        try c.encode(triggers, forKey: .triggers)
        try c.encode(content, forKey: .content)
        try c.encodeIfPresent(formatting, forKey: .formatting)
        try c.encodeUUIDIfPresent(groupID, forKey: .groupID)
        try c.encode(fireMode, forKey: .fireMode)
        try c.encode(caseMode, forKey: .caseMode)
        try c.encodeIfPresent(insertion, forKey: .insertion)
        try c.encodeIfPresent(rule, forKey: .rule)
        try c.encode(isFavorite, forKey: .isFavorite)
        try c.encode(isEnabled, forKey: .isEnabled)
        try c.encodeWholeSeconds(createdAt, forKey: .createdAt)
        try c.encodeWholeSeconds(updatedAt, forKey: .updatedAt)
    }
}

/// The fields a new snippet starts with (`Partial<Snippet>` passed to `api.createSnippet`).
/// `nil` keeps the default; `id`, `createdAt` and `updatedAt` are always chosen by the backend.
public struct SnippetDraft: Hashable, Sendable {
    public var name: String?
    public var triggers: [String]?
    public var content: String?
    public var formatting: Formatting?
    public var groupID: UUID?
    public var fireMode: FireMode?
    public var caseMode: CaseMode?
    public var insertion: InsertionMethod?
    public var rule: AppRule?
    public var isFavorite: Bool?
    public var isEnabled: Bool?

    public init(
        name: String? = nil,
        triggers: [String]? = nil,
        content: String? = nil,
        formatting: Formatting? = nil,
        groupID: UUID? = nil,
        fireMode: FireMode? = nil,
        caseMode: CaseMode? = nil,
        insertion: InsertionMethod? = nil,
        rule: AppRule? = nil,
        isFavorite: Bool? = nil,
        isEnabled: Bool? = nil
    ) {
        self.name = name
        self.triggers = triggers
        self.content = content
        self.formatting = formatting
        self.groupID = groupID
        self.fireMode = fireMode
        self.caseMode = caseMode
        self.insertion = insertion
        self.rule = rule
        self.isFavorite = isFavorite
        self.isEnabled = isEnabled
    }

    /// A new snippet stamped `now` with this draft's fields over `SnippetRecord::new`'s defaults.
    public func snippet(id: UUID = UUID(), now: Date = Date()) -> Snippet {
        Snippet(
            id: id,
            name: name ?? "",
            triggers: triggers ?? [],
            content: content ?? "",
            formatting: formatting,
            groupID: groupID,
            fireMode: fireMode ?? .immediate,
            caseMode: caseMode ?? .propagate,
            insertion: insertion,
            rule: rule,
            isFavorite: isFavorite ?? false,
            isEnabled: isEnabled ?? true,
            createdAt: now,
            updatedAt: now
        )
    }
}
