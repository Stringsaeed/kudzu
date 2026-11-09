import Foundation

// Port of the enums and `AppRule` in `kudzu-core/src/types.rs`.

/// How the case of a typed trigger is reflected in the expansion.
public enum CaseMode: String, Codable, Hashable, Sendable, CaseIterable {
    /// `;sig` → "Best regards", `;Sig` → "Best regards", `;SIG` → "BEST REGARDS".
    case propagate
    /// Trigger must match exactly as written; output is never altered.
    case exact
}

/// When a snippet fires relative to word boundaries.
public enum FireMode: String, Codable, Hashable, Sendable, CaseIterable {
    /// Fires as soon as the last trigger character is typed.
    case immediate
    /// Fires only when the trigger is a whole word, on the following separator (which is kept).
    case wholeWord
}

/// How expanded text is delivered to the focused app.
public enum InsertionMethod: String, Codable, Hashable, Sendable, CaseIterable {
    /// Short plain text as keystrokes, long text via paste.
    case automatic
    case type
    case paste
}

/// Whether an app rule lists the only apps a snippet works in, or the apps it never works in.
public enum AppRuleMode: String, Codable, Hashable, Sendable, CaseIterable {
    /// Active only in the listed apps.
    case allow
    /// Active everywhere except the listed apps.
    case deny
}

/// Per-app allow/deny list attached to a snippet or group.
public struct AppRule: Codable, Hashable, Sendable {
    public var mode: AppRuleMode
    public var bundleIDs: [String]
    /// Per-app insertion override; `nil` inherits from the parent scope.
    public var insertion: InsertionMethod?

    public init(mode: AppRuleMode, bundleIDs: [String] = [], insertion: InsertionMethod? = nil) {
        self.mode = mode
        self.bundleIDs = bundleIDs
        self.insertion = insertion
    }

    private enum CodingKeys: String, CodingKey { case mode, bundleIDs, insertion }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        mode = try c.decode(AppRuleMode.self, forKey: .mode)
        bundleIDs = try c.decode([String].self, forKey: .bundleIDs, default: [])
        insertion = try c.decodeIfPresent(InsertionMethod.self, forKey: .insertion)
    }
}
