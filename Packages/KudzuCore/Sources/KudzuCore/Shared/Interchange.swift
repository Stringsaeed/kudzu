import Foundation

// Plain value types shared by every KudzuCore layer.
//
// These are the contract between the persistence layer (SwiftData models),
// the expansion engine, and import/export. Persistence converts its @Model
// types to and from these; the engine and interchange code never touch
// SwiftData directly. Kudzu JSON export is a direct Codable encoding of
// `KudzuLibrary`, so adding fields must stay backward compatible
// (new fields optional or defaulted).

/// How the case of a typed trigger is reflected in the expansion.
public enum CaseMode: String, Codable, Sendable, CaseIterable {
    /// `;sig` → "Best regards", `;Sig` → "Best regards", `;SIG` → "BEST REGARDS".
    case propagate
    /// Trigger must match exactly as written; output is never altered.
    case exact
}

/// When a snippet fires relative to word boundaries.
public enum FireMode: String, Codable, Sendable, CaseIterable {
    /// Fires as soon as the last trigger character is typed.
    case immediate
    /// Fires only when the trigger is a whole word: preceded by a boundary
    /// (or start of buffer) and followed by a separator, which is kept.
    case wholeWord
}

/// How expanded text is delivered to the focused app.
public enum InsertionMethod: String, Codable, Sendable, CaseIterable {
    /// Short plain text as keystrokes, long or rich text via paste.
    case automatic
    case type
    case paste
}

public enum AppRuleMode: String, Codable, Sendable, CaseIterable {
    /// Active only in the listed apps.
    case allow
    /// Active everywhere except the listed apps.
    case deny
}

public struct AppRule: Codable, Hashable, Sendable {
    public var mode: AppRuleMode
    public var bundleIDs: [String]
    /// Per-app insertion override; nil inherits from the parent scope.
    public var insertion: InsertionMethod?

    public init(mode: AppRuleMode, bundleIDs: [String], insertion: InsertionMethod? = nil) {
        self.mode = mode
        self.bundleIDs = bundleIDs
        self.insertion = insertion
    }
}

public struct SnippetRecord: Codable, Hashable, Sendable, Identifiable {
    public var id: UUID
    public var name: String
    /// One or more triggers. Never empty for an enabled snippet.
    public var triggers: [String]
    /// Template source; see docs/TEMPLATE_SYNTAX.md.
    public var content: String
    public var groupID: UUID?
    public var fireMode: FireMode
    public var caseMode: CaseMode
    /// nil inherits from the group, then the global default.
    public var insertion: InsertionMethod?
    /// nil inherits the group's rule.
    public var rule: AppRule?
    public var isFavorite: Bool
    public var isEnabled: Bool
    public var createdAt: Date
    public var updatedAt: Date

    public init(
        id: UUID = UUID(),
        name: String,
        triggers: [String],
        content: String,
        groupID: UUID? = nil,
        fireMode: FireMode = .immediate,
        caseMode: CaseMode = .propagate,
        insertion: InsertionMethod? = nil,
        rule: AppRule? = nil,
        isFavorite: Bool = false,
        isEnabled: Bool = true,
        createdAt: Date = .now,
        updatedAt: Date = .now
    ) {
        self.id = id
        self.name = name
        self.triggers = triggers
        self.content = content
        self.groupID = groupID
        self.fireMode = fireMode
        self.caseMode = caseMode
        self.insertion = insertion
        self.rule = rule
        self.isFavorite = isFavorite
        self.isEnabled = isEnabled
        self.createdAt = createdAt
        self.updatedAt = updatedAt
    }
}

public struct GroupRecord: Codable, Hashable, Sendable, Identifiable {
    public var id: UUID
    public var name: String
    public var parentID: UUID?
    public var sortIndex: Int
    public var isEnabled: Bool
    public var insertion: InsertionMethod?
    public var rule: AppRule?

    public init(
        id: UUID = UUID(),
        name: String,
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
}

public struct UsageRecord: Codable, Hashable, Sendable {
    public var snippetID: UUID
    public var deviceID: String
    public var count: Int
    public var charsSaved: Int
    public var lastUsedAt: Date?

    public init(snippetID: UUID, deviceID: String, count: Int = 0, charsSaved: Int = 0, lastUsedAt: Date? = nil) {
        self.snippetID = snippetID
        self.deviceID = deviceID
        self.count = count
        self.charsSaved = charsSaved
        self.lastUsedAt = lastUsedAt
    }
}

/// A whole library: the unit of import, export and backup.
public struct KudzuLibrary: Codable, Hashable, Sendable {
    public static let currentFormatVersion = 1

    public var formatVersion: Int
    public var groups: [GroupRecord]
    public var snippets: [SnippetRecord]
    public var usage: [UsageRecord]

    public init(
        formatVersion: Int = KudzuLibrary.currentFormatVersion,
        groups: [GroupRecord] = [],
        snippets: [SnippetRecord] = [],
        usage: [UsageRecord] = []
    ) {
        self.formatVersion = formatVersion
        self.groups = groups
        self.snippets = snippets
        self.usage = usage
    }
}
