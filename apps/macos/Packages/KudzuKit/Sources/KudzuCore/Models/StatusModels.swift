import Foundation

// Status types of the Tauri shell: `cloud/mod.rs` (`SyncStatus`), `updater.rs` (`UpdateState`)
// and `agent.rs` (`AgentSetup`). Timestamps inside them are strings, exactly as Rust sends them.

/// iCloud sync status as the UI shows it. JSON is tagged by `state`.
public enum SyncStatus: Codable, Hashable, Sendable {
    /// Turned off in Settings.
    case off
    /// This build can't use iCloud (unsigned / dev build).
    case unavailable(reason: String)
    /// No iCloud account, or iCloud Drive / CloudKit restricted on this Mac.
    case noAccount(reason: String)
    case starting
    case syncing
    case upToDate(lastSync: String)
    /// Another Mac uses a newer sync format; this one stopped sending until it updates.
    case needsUpdate
    case error(message: String)

    private enum CodingKeys: String, CodingKey { case state, reason, lastSync, message }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .state) {
        case "off": self = .off
        case "unavailable": self = .unavailable(reason: try c.decode(String.self, forKey: .reason))
        case "noAccount": self = .noAccount(reason: try c.decode(String.self, forKey: .reason))
        case "starting": self = .starting
        case "syncing": self = .syncing
        case "upToDate": self = .upToDate(lastSync: try c.decode(String.self, forKey: .lastSync))
        case "needsUpdate": self = .needsUpdate
        case "error": self = .error(message: try c.decode(String.self, forKey: .message))
        case let other: throw DecodingError.dataCorruptedError(forKey: .state, in: c, debugDescription: "unknown sync state \(other)")
        }
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .off: try c.encode("off", forKey: .state)
        case let .unavailable(reason):
            try c.encode("unavailable", forKey: .state)
            try c.encode(reason, forKey: .reason)
        case let .noAccount(reason):
            try c.encode("noAccount", forKey: .state)
            try c.encode(reason, forKey: .reason)
        case .starting: try c.encode("starting", forKey: .state)
        case .syncing: try c.encode("syncing", forKey: .state)
        case let .upToDate(lastSync):
            try c.encode("upToDate", forKey: .state)
            try c.encode(lastSync, forKey: .lastSync)
        case .needsUpdate: try c.encode("needsUpdate", forKey: .state)
        case let .error(message):
            try c.encode("error", forKey: .state)
            try c.encode(message, forKey: .message)
        }
    }
}

/// What the UI shows about updates. JSON is tagged by `phase`; `Option`s are written as `null`.
public enum UpdateState: Codable, Hashable, Sendable {
    case idle
    case checking
    case upToDate(checkedAt: String)
    case downloading(version: String, downloaded: UInt64, total: UInt64?)
    /// Downloaded and verified; installs on quit or on "Restart to Update".
    case ready(version: String, notes: String?, date: String?)
    /// Installing; the app relaunches in a moment.
    case restarting(version: String)
    /// `needsMove` is set when the failure is about where the app lives, not the network.
    case error(message: String, needsMove: Bool)

    private enum CodingKeys: String, CodingKey { case phase, checkedAt, version, downloaded, total, notes, date, message, needsMove }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .phase) {
        case "idle": self = .idle
        case "checking": self = .checking
        case "upToDate": self = .upToDate(checkedAt: try c.decode(String.self, forKey: .checkedAt))
        case "downloading":
            self = .downloading(
                version: try c.decode(String.self, forKey: .version),
                downloaded: try c.decode(UInt64.self, forKey: .downloaded),
                total: try c.decodeIfPresent(UInt64.self, forKey: .total)
            )
        case "ready":
            self = .ready(
                version: try c.decode(String.self, forKey: .version),
                notes: try c.decodeIfPresent(String.self, forKey: .notes),
                date: try c.decodeIfPresent(String.self, forKey: .date)
            )
        case "restarting": self = .restarting(version: try c.decode(String.self, forKey: .version))
        case "error": self = .error(message: try c.decode(String.self, forKey: .message), needsMove: try c.decode(Bool.self, forKey: .needsMove))
        case let other: throw DecodingError.dataCorruptedError(forKey: .phase, in: c, debugDescription: "unknown update phase \(other)")
        }
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .idle: try c.encode("idle", forKey: .phase)
        case .checking: try c.encode("checking", forKey: .phase)
        case let .upToDate(checkedAt):
            try c.encode("upToDate", forKey: .phase)
            try c.encode(checkedAt, forKey: .checkedAt)
        case let .downloading(version, downloaded, total):
            try c.encode("downloading", forKey: .phase)
            try c.encode(version, forKey: .version)
            try c.encode(downloaded, forKey: .downloaded)
            try c.encodeNullable(total, forKey: .total)
        case let .ready(version, notes, date):
            try c.encode("ready", forKey: .phase)
            try c.encode(version, forKey: .version)
            try c.encodeNullable(notes, forKey: .notes)
            try c.encodeNullable(date, forKey: .date)
        case let .restarting(version):
            try c.encode("restarting", forKey: .phase)
            try c.encode(version, forKey: .version)
        case let .error(message, needsMove):
            try c.encode("error", forKey: .phase)
            try c.encode(message, forKey: .message)
            try c.encode(needsMove, forKey: .needsMove)
        }
    }
}

/// How to connect an MCP client to this copy of Kudzu.
public struct AgentSetup: Codable, Hashable, Sendable {
    /// Whether the skill file at ``AgentSetup/skillPath`` matches this build's.
    public enum SkillState: String, Codable, Hashable, Sendable {
        case missing, current, outdated
    }

    public var running: Bool
    /// The MCP server name to register ("kudzu", or "kudzu-dev").
    public var name: String
    public var command: String
    public var args: [String]
    /// A ready-to-paste `claude mcp add` command.
    public var claudeCommand: String
    /// `mcpServers` JSON for other MCP clients.
    public var configJson: String
    public var skillPath: String
    public var skill: SkillState
    /// The skill's folder inside the app bundle (release builds), for linking into other agents.
    public var bundledSkillDir: String?

    public init(
        running: Bool,
        name: String,
        command: String,
        args: [String],
        claudeCommand: String,
        configJson: String,
        skillPath: String,
        skill: SkillState,
        bundledSkillDir: String?
    ) {
        self.running = running
        self.name = name
        self.command = command
        self.args = args
        self.claudeCommand = claudeCommand
        self.configJson = configJson
        self.skillPath = skillPath
        self.skill = skill
        self.bundledSkillDir = bundledSkillDir
    }

    private enum CodingKeys: String, CodingKey {
        case running, name, command, args, claudeCommand, configJson, skillPath, skill, bundledSkillDir
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(running, forKey: .running)
        try c.encode(name, forKey: .name)
        try c.encode(command, forKey: .command)
        try c.encode(args, forKey: .args)
        try c.encode(claudeCommand, forKey: .claudeCommand)
        try c.encode(configJson, forKey: .configJson)
        try c.encode(skillPath, forKey: .skillPath)
        try c.encode(skill, forKey: .skill)
        try c.encodeNullable(bundledSkillDir, forKey: .bundledSkillDir)
    }
}
