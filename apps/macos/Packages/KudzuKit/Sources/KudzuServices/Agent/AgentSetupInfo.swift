import Foundation
import KudzuCore

/// Settings ▸ AI agents: how to connect an agent to this copy of Kudzu, and the Claude Code
/// skill (`agent_setup`, `install_agent_skill` in `agent.rs`).
public enum AgentSetupInfo {
    /// `~/.claude/skills/kudzu/SKILL.md`.
    public static var skillURL: URL {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/skills/kudzu/SKILL.md")
    }

    /// The setup for `identity`, launched as `executable`.
    public static func make(
        identity: AppIdentity,
        running: Bool,
        executable: String = Bundle.main.executableURL?.path ?? CommandLine.arguments.first ?? "",
        skillURL: URL = AgentSetupInfo.skillURL,
        bundledSkillDir: URL? = AgentSetupInfo.bundledSkillDir()
    ) -> AgentSetup {
        let dev = identity.isDev
        let name = dev ? "kudzu-dev" : "kudzu"
        var args = ["--mcp"]
        if dev { args.append("--dev") }
        let claude = "claude mcp add --scope user \(name) -- \(shellQuote(executable)) \(args.joined(separator: " "))"
        let skill: AgentSetup.SkillState
        switch try? String(contentsOf: skillURL, encoding: .utf8) {
        case let s? where s == Agent.SKILL: skill = .current
        case .some: skill = .outdated
        case nil: skill = .missing
        }
        return AgentSetup(
            running: running,
            name: name,
            command: executable,
            args: args,
            claudeCommand: claude,
            configJson: configJSON(name: name, command: executable, args: args),
            skillPath: skillURL.path,
            skill: skill,
            bundledSkillDir: bundledSkillDir?.path
        )
    }

    /// The skill's folder inside the app bundle (release builds), for linking into other agents.
    public static func bundledSkillDir(bundle: Bundle = .main) -> URL? {
        guard let dir = bundle.resourceURL?.appendingPathComponent("skills/kudzu", isDirectory: true) else { return nil }
        return FileManager.default.fileExists(atPath: dir.appendingPathComponent("SKILL.md").path) ? dir : nil
    }

    /// Writes the Kudzu skill to `~/.claude/skills/kudzu/SKILL.md` and returns its path.
    public static func installSkill(at url: URL = AgentSetupInfo.skillURL) throws -> String {
        try installSkill(text: Agent.SKILL, at: url)
    }

    /// Writes `text` as the skill file. An empty catalog is refused so a missing resource
    /// cannot replace a skill the user already has.
    static func installSkill(text: String, at url: URL) throws -> String {
        guard !text.isEmpty else { throw ServiceError("The Kudzu skill text failed to load, so it was not installed.") }
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try AtomicFile.write(Data(text.utf8), to: url)
        return url.path
    }

    /// `agent::shell_quote`: bare when safe, otherwise single-quoted.
    public static func shellQuote(_ s: String) -> String {
        let safe = Set("/._-+=:,@".utf8)
        if !s.isEmpty, s.utf8.allSatisfy({ ($0 < 128 && (isalnum(Int32($0)) != 0)) || safe.contains($0) }) {
            return s
        }
        return "'" + s.replacingOccurrences(of: "'", with: #"'\''"#) + "'"
    }

    /// `mcpServers` JSON for other MCP clients, formatted like `serde_json::to_string_pretty`
    /// (two-space indent, `"key": value`, keys sorted).
    public static func configJSON(name: String, command: String, args: [String]) -> String {
        let argLines = args.map { "        \(jsonString($0))" }.joined(separator: ",\n")
        let argsBlock = args.isEmpty ? "[]" : "[\n\(argLines)\n      ]"
        return """
        {
          "mcpServers": {
            \(jsonString(name)): {
              "args": \(argsBlock),
              "command": \(jsonString(command))
            }
          }
        }
        """
    }

    static func jsonString(_ s: String) -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = .withoutEscapingSlashes
        return (try? encoder.encode(s)).map { String(decoding: $0, as: UTF8.self) } ?? "\"\""
    }
}
