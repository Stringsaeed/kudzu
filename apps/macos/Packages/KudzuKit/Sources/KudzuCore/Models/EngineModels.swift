import Foundation

// Value types of `kudzu-engine/src/lib.rs` (the engine's public contract). serde writes `Option`
// fields here as `null` (no `skip_serializing_if`), so the encoders do too.

/// A screen rectangle in global top-left-origin points (logical pixels). Rust `Rect`.
public struct ScreenRect: Codable, Hashable, Sendable {
    public var x: Double
    public var y: Double
    public var width: Double
    public var height: Double

    public init(x: Double, y: Double, width: Double, height: Double) {
        self.x = x
        self.y = y
        self.width = width
        self.height = height
    }
}

/// A running or installed application.
public struct AppInfo: Codable, Hashable, Sendable {
    public var bundleId: String
    public var name: String
    /// Process id when running.
    public var pid: Int32?
    /// Bundle path on disk, when known.
    public var path: String?

    public init(bundleId: String, name: String, pid: Int32? = nil, path: String? = nil) {
        self.bundleId = bundleId
        self.name = name
        self.pid = pid
        self.path = path
    }

    private enum CodingKeys: String, CodingKey { case bundleId, name, pid, path }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(bundleId, forKey: .bundleId)
        try c.encode(name, forKey: .name)
        try c.encodeNullable(pid, forKey: .pid)
        try c.encodeNullable(path, forKey: .path)
    }
}

/// Privacy permissions the engine needs.
public struct PermissionStatus: Codable, Hashable, Sendable {
    /// Accessibility (post events, read caret position, active tap).
    public var accessibility: Bool
    /// Input Monitoring (listen-only tap).
    public var inputMonitoring: Bool

    public init(accessibility: Bool = false, inputMonitoring: Bool = false) {
        self.accessibility = accessibility
        self.inputMonitoring = inputMonitoring
    }
}

/// How the keyboard tap is currently attached.
public enum TapMode: String, Codable, Hashable, Sendable {
    /// Not listening (missing permissions or stopped).
    case off
    /// Can observe but not swallow keystrokes.
    case listenOnly
    /// Can observe and swallow keystrokes (best reliability).
    case active
}

/// Engine health for the tray and UI.
public struct EngineStatus: Codable, Hashable, Sendable {
    /// Permission to insert synthetic keystrokes, independent of pause/Secure Input.
    public var canPostEvents: Bool
    /// A live listener can insert, and neither pause nor Secure Input blocks it.
    public var canExpand: Bool
    public var paused: Bool
    public var tap: TapMode
    public var permissions: PermissionStatus
    public var secureInput: Bool
    /// App holding Secure Input, when known.
    public var secureInputOwner: String?
    /// One-line human summary ("Ready", "Paused", "Secure Input on — expansion blocked"…).
    public var summary: String

    public init(
        canPostEvents: Bool = false,
        canExpand: Bool = false,
        paused: Bool = false,
        tap: TapMode = .off,
        permissions: PermissionStatus = PermissionStatus(),
        secureInput: Bool = false,
        secureInputOwner: String? = nil,
        summary: String
    ) {
        self.canPostEvents = canPostEvents
        self.canExpand = canExpand
        self.paused = paused
        self.tap = tap
        self.permissions = permissions
        self.secureInput = secureInput
        self.secureInputOwner = secureInputOwner
        self.summary = summary
    }

    /// What the UI shows before the backend answers (`lib/store.ts` `defaultStatus`).
    public static let starting = EngineStatus(summary: "Starting…")

    private enum CodingKeys: String, CodingKey {
        case canPostEvents, canExpand, paused, tap, permissions, secureInput, secureInputOwner, summary
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        canPostEvents = try c.decode(Bool.self, forKey: .canPostEvents, default: false)
        canExpand = try c.decode(Bool.self, forKey: .canExpand, default: false)
        paused = try c.decode(Bool.self, forKey: .paused)
        tap = try c.decode(TapMode.self, forKey: .tap)
        permissions = try c.decode(PermissionStatus.self, forKey: .permissions)
        secureInput = try c.decode(Bool.self, forKey: .secureInput)
        secureInputOwner = try c.decodeIfPresent(String.self, forKey: .secureInputOwner)
        summary = try c.decode(String.self, forKey: .summary)
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(canPostEvents, forKey: .canPostEvents)
        try c.encode(canExpand, forKey: .canExpand)
        try c.encode(paused, forKey: .paused)
        try c.encode(tap, forKey: .tap)
        try c.encode(permissions, forKey: .permissions)
        try c.encode(secureInput, forKey: .secureInput)
        try c.encodeNullable(secureInputOwner, forKey: .secureInputOwner)
        try c.encode(summary, forKey: .summary)
    }
}

/// The engine needs a form filled before it can insert a snippet.
public struct FormRequest: Codable, Hashable, Sendable {
    /// Pass back to `submitForm` / `cancelForm`.
    public var token: UInt64
    public var snippetId: UUID
    public var snippetName: String
    /// Fields in first-appearance order, including nested snippets.
    public var fields: [FormFieldSpec]
    /// Caret rectangle in the target app, when the Accessibility API exposes it.
    public var caret: ScreenRect?
    /// The app the text will go into.
    public var targetApp: AppInfo?

    public init(token: UInt64, snippetId: UUID, snippetName: String, fields: [FormFieldSpec], caret: ScreenRect? = nil, targetApp: AppInfo? = nil) {
        self.token = token
        self.snippetId = snippetId
        self.snippetName = snippetName
        self.fields = fields
        self.caret = caret
        self.targetApp = targetApp
    }

    private enum CodingKeys: String, CodingKey { case token, snippetId, snippetName, fields, caret, targetApp }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(token, forKey: .token)
        try c.encodeUUID(snippetId, forKey: .snippetId)
        try c.encode(snippetName, forKey: .snippetName)
        try c.encode(fields, forKey: .fields)
        try c.encodeNullable(caret, forKey: .caret)
        try c.encodeNullable(targetApp, forKey: .targetApp)
    }
}

/// One successful expansion (for usage stats). Never contains typed or inserted text.
public struct ExpansionEvent: Codable, Hashable, Sendable {
    public var snippetId: UUID
    /// Inserted characters minus typed trigger characters (≥ 0).
    public var charsSaved: Int

    public init(snippetId: UUID, charsSaved: Int) {
        self.snippetId = snippetId
        self.charsSaved = charsSaved
    }

    private enum CodingKeys: String, CodingKey { case snippetId, charsSaved }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encodeUUID(snippetId, forKey: .snippetId)
        try c.encode(charsSaved, forKey: .charsSaved)
    }
}

// MARK: - Permissions (src-tauri/src/permissions)

/// The privacy permission a guide or request is about.
public enum PermissionKind: String, Codable, Hashable, Sendable {
    case accessibility
    case inputMonitoring
}

/// A System Settings pane `openSystemSettings` can open.
public enum SystemSettingsPane: String, Codable, Hashable, Sendable {
    case accessibility
    case inputMonitoring
    case loginItems
}

/// What the permission companion window shows (`permission_context`).
public struct PermissionContext: Codable, Hashable, Sendable {
    public var kind: PermissionKind
    /// The app's product name ("Kudzu" or "Kudzu Dev").
    public var appName: String
    /// "Accessibility", or "Device Control and Data Access" on macOS versions that renamed it.
    public var accessibilityName: String

    public init(kind: PermissionKind, appName: String, accessibilityName: String) {
        self.kind = kind
        self.appName = appName
        self.accessibilityName = accessibilityName
    }
}
