import Foundation

// `View` and `SettingsSection` of `lib/store.ts`. The TypeScript name `View` would shadow
// SwiftUI's `View`, so the main window's current screen is a `Route` here.

/// A page of the Settings screen.
public enum SettingsSection: String, Codable, Hashable, Sendable, CaseIterable {
    case general, expansion, clipboard, agents, apps, data, shortcuts, about
}

/// What the main window shows. JSON is tagged by `kind` (`{"kind":"group","id":…}`), as the
/// `navigate` event and `show_main_view` carry it.
public enum Route: Codable, Hashable, Sendable {
    case all
    case favorites
    case recent
    case disabled
    case group(id: UUID)
    case insights
    case clipboard(filter: ClipFilter? = nil)
    case settings(section: SettingsSection? = nil)

    /// The `kind` discriminator.
    public var kind: String {
        switch self {
        case .all: "all"
        case .favorites: "favorites"
        case .recent: "recent"
        case .disabled: "disabled"
        case .group: "group"
        case .insights: "insights"
        case .clipboard: "clipboard"
        case .settings: "settings"
        }
    }

    private enum CodingKeys: String, CodingKey { case kind, id, filter, section }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .kind) {
        case "all": self = .all
        case "favorites": self = .favorites
        case "recent": self = .recent
        case "disabled": self = .disabled
        case "group": self = .group(id: try c.decode(UUID.self, forKey: .id))
        case "insights": self = .insights
        case "clipboard": self = .clipboard(filter: try c.decodeIfPresent(ClipFilter.self, forKey: .filter))
        case "settings": self = .settings(section: try c.decodeIfPresent(SettingsSection.self, forKey: .section))
        case let other: throw DecodingError.dataCorruptedError(forKey: .kind, in: c, debugDescription: "unknown view \(other)")
        }
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(kind, forKey: .kind)
        switch self {
        case let .group(id): try c.encodeUUID(id, forKey: .id)
        case let .clipboard(filter): try c.encodeIfPresent(filter, forKey: .filter)
        case let .settings(section): try c.encodeIfPresent(section, forKey: .section)
        case .all, .favorites, .recent, .disabled, .insights: break
        }
    }
}
