import Foundation

// IPC payloads of the Tauri shell (`commands.rs`, `state.rs`) that every window reads.

/// The OS the backend runs on (`get_state`'s `platform`; `web` is the browser preview).
public enum Platform: String, Codable, Hashable, Sendable {
    case macos, windows, linux, web
}

/// Everything a window needs at startup (`get_state`).
public struct AppState: Codable, Hashable, Sendable {
    public var library: Library
    public var settings: Settings
    public var status: EngineStatus
    public var platform: Platform

    public init(library: Library, settings: Settings, status: EngineStatus, platform: Platform) {
        self.library = library
        self.settings = settings
        self.status = status
        self.platform = platform
    }
}

/// A form for the form panel to show (Rust `FormPayload`, the TypeScript `FormRequest`): the engine's
/// request plus remembered values and the snippet source for the live preview.
public struct FormPayload: Codable, Hashable, Sendable {
    /// Pass back to `submitForm` / `cancelForm`.
    public var token: UInt64
    public var snippetId: UUID
    public var snippetName: String
    public var fields: [FormFieldSpec]
    /// Remembered (or default) value per field name.
    public var initialValues: [String: String]
    /// The snippet's template source.
    public var content: String
    public var targetApp: AppInfo?

    public init(
        token: UInt64,
        snippetId: UUID,
        snippetName: String,
        fields: [FormFieldSpec],
        initialValues: [String: String] = [:],
        content: String,
        targetApp: AppInfo? = nil
    ) {
        self.token = token
        self.snippetId = snippetId
        self.snippetName = snippetName
        self.fields = fields
        self.initialValues = initialValues
        self.content = content
        self.targetApp = targetApp
    }

    private enum CodingKeys: String, CodingKey { case token, snippetId, snippetName, fields, initialValues, content, targetApp }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(token, forKey: .token)
        try c.encodeUUID(snippetId, forKey: .snippetId)
        try c.encode(snippetName, forKey: .snippetName)
        try c.encode(fields, forKey: .fields)
        try c.encode(initialValues, forKey: .initialValues)
        try c.encode(content, forKey: .content)
        try c.encodeNullable(targetApp, forKey: .targetApp)
    }
}
