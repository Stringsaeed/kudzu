import Foundation

/// A warning emitted during an Espanso import for unsupported or degraded features.
public struct ImportWarning: Codable, Hashable, Sendable {
    public var file: String
    public var line: Int?
    public var matchIndex: Int?
    public var trigger: String?
    public var message: String

    public init(
        file: String,
        line: Int? = nil,
        matchIndex: Int? = nil,
        trigger: String? = nil,
        message: String
    ) {
        self.file = file
        self.line = line
        self.matchIndex = matchIndex
        self.trigger = trigger
        self.message = message
    }
}

/// The result of importing an Espanso configuration or match files.
public struct EspansoImportResult: Codable, Hashable, Sendable {
    public var library: KudzuLibrary
    public var warnings: [ImportWarning]

    public init(library: KudzuLibrary, warnings: [ImportWarning] = []) {
        self.library = library
        self.warnings = warnings
    }
}
