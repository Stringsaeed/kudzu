import Foundation
import KudzuCore

// The rest of `MockBackend.invoke`: templates, apps, data, windows, updates, sync, agents and
// clipboard history. Commands the TypeScript mock answered with `undefined` are no-ops here.

extension MockBackend {
    // MARK: Templates

    public func renderPreview(content: String, snippetId: UUID?, formValues: [String: String], formatting: Formatting?) -> RenderResult {
        let library = state.library
        let sources = Dictionary(library.snippets.map { ($0.id, $0.content) }, uniquingKeysWith: { a, _ in a })
        let formats = Dictionary(library.snippets.compactMap { s in s.formatting.map { (s.id, $0) } }, uniquingKeysWith: { a, _ in a })
        let context = RenderContext(
            now: Date(),
            timeZone: .current,
            clipboard: "copied text",
            formValues: formValues,
            resolveSnippet: { sources[$0] },
            resolveFormatting: { formats[$0] },
            rootSnippetID: snippetId
        )
        return Template.renderFormatted(content, formatting: formatting, context: context)
    }

    public func copyPreview(content: String, snippetId: UUID?, formatting: Formatting?) {}

    // MARK: Apps

    public func listApps() -> [AppInfo] { MockSeed.apps }
    public func appIcon(path: String) -> Data? { nil }

    // MARK: Import / export / backups

    public func detectEspanso() -> [EspansoDetection] {
        guard options.espanso != .none else { return [] }
        return [
            EspansoDetection(path: "preview://espanso/work", snippetCount: 3, groupCount: 1),
            EspansoDetection(path: "preview://espanso/personal", snippetCount: 2, groupCount: 1),
        ]
    }

    public func importEspanso(path: String) throws -> ImportSummary {
        guard path == "preview://espanso/work" || path == "preview://espanso/personal" else {
            throw BackendError("Import is available in the desktop app.")
        }
        if path.hasSuffix("personal"), options.espanso == .retry, !importPreviewFailed {
            importPreviewFailed = true
            throw BackendError("Preview: the second source is temporarily unavailable.")
        }
        return ImportSummary(importedSnippets: path.hasSuffix("work") ? 3 : 2, importedGroups: 1)
    }

    public func importFile(path: String) throws -> ImportSummary {
        guard path.hasPrefix("preview://") else { throw BackendError("Import is available in the desktop app.") }
        let raycast = path == "preview://raycast"
        // Importing a sample a second time merges everything into what is already there.
        let again = !importedPreviews.insert("import:\(path)").inserted
        let count = raycast ? 4 : 6
        return ImportSummary(
            importedSnippets: count,
            importedGroups: 1,
            dedupeReport: DedupeReport(mergedSnippets: again ? count : 0),
            warnings: raycast ? [] : [ImportWarning(file: "export.csv", message: "Script snippet isn’t supported and was skipped.")]
        )
    }

    public func exportJson(path: String) {}
    public func exportEspanso(dir: String) -> Int { 0 }
    public func listBackups() -> [BackupInfo] { MockSeed.backups() }
    public func backupNow() -> BackupInfo {
        BackupInfo(path: "/backups/manual.json", filename: "kudzu-manual.json", createdAt: MockSeed.iso(), reason: .manual)
    }
    public func restoreBackup(path: String) {}
    public func revealDataFolder() {}

    // MARK: Windows

    public func showMain(snippetId: UUID?) {}
    public func hidePalette() {}
    public func paletteInsert(snippetId: UUID) {}
    public func paletteCopy(snippetId: UUID) {}
    public func paletteCreate(query: String) {}
    public func submitForm(token: UInt64, values: [String: String]) {}
    public func cancelForm(token: UInt64) {}
    public func resizeForm(height: Double) {}

    public func pendingForm() -> FormPayload? {
        let library = state.library
        guard options.demoForm, let s = library.snippets.first(where: { $0.content.contains("{{field") }) else { return nil }
        let sources = Dictionary(library.snippets.map { ($0.id, $0.content) }, uniquingKeysWith: { a, _ in a })
        return FormPayload(
            token: 1,
            snippetId: s.id,
            snippetName: s.name,
            fields: Template.formFields(s.content) { sources[$0] },
            content: s.content,
            targetApp: AppInfo(bundleId: "com.apple.mail", name: "Mail")
        )
    }

    // MARK: Updates (`?update` shows an available version; otherwise "up to date" after a short check)

    public func appVersion() -> String { "0.1.0" }

    public func updateState() -> UpdateState {
        options.update ? readyUpdate() : .idle
    }

    public func checkForUpdates() async throws -> UpdateState {
        try await Task.sleep(for: options.checkDelay)
        return options.update ? readyUpdate() : .upToDate(checkedAt: KudzuDate.formatMilliseconds(Date()))
    }

    private func readyUpdate() -> UpdateState {
        .ready(version: "0.2.0", notes: MockSeed.updateNotes, date: KudzuDate.formatMilliseconds(Date()))
    }

    public func restartToUpdate() {}

    // MARK: iCloud sync

    public func syncStatus() -> SyncStatus {
        if !state.settings.icloudSync { return .off }
        if options.noSync {
            return .unavailable(reason: "This copy of Kudzu isn't signed for iCloud. Release builds sync; development builds stay on this Mac.")
        }
        return .upToDate(lastSync: KudzuDate.formatMilliseconds(Date().addingTimeInterval(-120)))
    }

    public func syncNow() {}

    // MARK: AI agents

    public func agentSetup() -> AgentSetup {
        let command = "/Applications/Kudzu.app/Contents/MacOS/kudzu-desktop"
        let config = "{\n  \"mcpServers\": {\n    \"kudzu\": {\n      \"command\": \"\(command)\",\n      \"args\": [\n        \"--mcp\"\n      ]\n    }\n  }\n}"
        return AgentSetup(
            running: state.settings.agentAccess,
            name: "kudzu",
            command: command,
            args: ["--mcp"],
            claudeCommand: "claude mcp add --scope user kudzu -- \(command) --mcp",
            configJson: config,
            skillPath: "~/.claude/skills/kudzu/SKILL.md",
            skill: skillInstalled ? .current : .missing,
            bundledSkillDir: "/Applications/Kudzu.app/Contents/Resources/skills/kudzu"
        )
    }

    public func installAgentSkill() -> String {
        skillInstalled = true
        return "~/.claude/skills/kudzu/SKILL.md"
    }
}
