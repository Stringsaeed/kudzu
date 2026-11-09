import Foundation
import KudzuCore

/// Everything the UI asks of the app: the port of `api` in `apps/desktop/lib/backend.ts`
/// (one method per Tauri command, same names in camelCase, typed results), plus the events
/// the backend pushes (`Events`).
///
/// `MockBackend` answers in previews and tests; `LiveBackend` (T6) wires the services.
/// Errors carry the user-facing message the Tauri command returned.
public protocol Backend: Sendable {
    /// Backend events. Every access subscribes anew, so each window/model gets every event.
    var events: AsyncStream<BackendEvent> { get }

    func getState() async throws -> AppState

    // MARK: Library

    func createSnippet(draft: SnippetDraft) async throws -> Snippet
    func saveSnippet(_ snippet: Snippet) async throws -> Snippet
    func deleteSnippets(ids: [UUID]) async throws
    func restoreSnippets(_ snippets: [Snippet]) async throws
    func duplicateSnippet(id: UUID) async throws -> Snippet
    func moveSnippets(ids: [UUID], groupId: UUID?) async throws
    func saveGroup(_ group: Group) async throws -> Group
    func createGroup(name: String, parentId: UUID?) async throws -> Group
    func deleteGroup(id: UUID, deleteSnippets: Bool) async throws
    func moveGroup(id: UUID, parentId: UUID?, index: Int) async throws
    func findConflicts() async throws -> [TriggerConflict]

    // MARK: Settings & engine

    func showPermissions(kind: PermissionKind) async throws
    func permissionContext() async throws -> PermissionContext
    /// Places the draggable app icon over `bounds` of the companion window; `false` when unsupported.
    func installPermissionIcon(bounds: ScreenRect) async throws -> Bool
    func beginPermission(kind: PermissionKind) async throws
    func stopPermissionGuide() async throws
    func restartPermissionApp() async throws
    func closePermissions() async throws
    func revealPermissionApp() async throws
    func updateSettings(_ patch: SettingsPatch) async throws -> Settings
    func setPaused(_ paused: Bool) async throws -> EngineStatus
    func requestPermission(kind: PermissionKind) async throws
    func openSystemSettings(pane: SystemSettingsPane) async throws
    func engineStatus() async throws -> EngineStatus

    // MARK: Templates

    func renderPreview(content: String, snippetId: UUID?, formValues: [String: String], formatting: Formatting?) async throws -> RenderResult
    func copyPreview(content: String, snippetId: UUID?, formatting: Formatting?) async throws

    // MARK: Apps

    func listApps() async throws -> [AppInfo]
    /// PNG data of the app's icon (the Tauri command returned it as a data URL).
    func appIcon(path: String) async throws -> Data?

    // MARK: Import / export / backups

    func detectEspanso() async throws -> [EspansoDetection]
    func importEspanso(path: String) async throws -> ImportSummary
    func importFile(path: String) async throws -> ImportSummary
    func exportJson(path: String) async throws
    /// Resolves to the number of snippets written.
    func exportEspanso(dir: String) async throws -> Int
    func listBackups() async throws -> [BackupInfo]
    func backupNow() async throws -> BackupInfo
    func restoreBackup(path: String) async throws
    func revealDataFolder() async throws

    // MARK: Windows

    func showMain(snippetId: UUID?) async throws
    func hidePalette() async throws
    func paletteInsert(snippetId: UUID) async throws
    func paletteCopy(snippetId: UUID) async throws
    func paletteCreate(query: String) async throws
    func setPaletteHotkey(accelerator: String) async throws -> Settings
    func pendingForm() async throws -> FormPayload?
    func submitForm(token: UInt64, values: [String: String]) async throws
    func cancelForm(token: UInt64) async throws
    func resizeForm(height: Double) async throws

    // MARK: Updates

    func appVersion() async throws -> String
    func updateState() async throws -> UpdateState
    func checkForUpdates() async throws -> UpdateState
    func restartToUpdate() async throws

    // MARK: iCloud sync

    func syncStatus() async throws -> SyncStatus
    func syncNow() async throws

    // MARK: AI agents (MCP)

    func agentSetup() async throws -> AgentSetup
    /// Writes the Claude Code skill; resolves to its path.
    func installAgentSkill() async throws -> String

    // MARK: Clipboard history

    func clipboardList() async throws -> ClipboardInfo
    func clipboardText(id: UUID) async throws -> String?
    func clipboardCopy(id: UUID, plain: Bool) async throws
    /// From the picker: pasted into the previous app, or only copied (no Accessibility).
    func clipboardPaste(id: UUID, plain: Bool) async throws -> PasteOutcome
    func clipboardDelete(ids: [UUID]) async throws -> [ClipEntry]
    func clipboardRestore(_ entries: [ClipEntry]) async throws
    func clipboardPin(id: UUID, pinned: Bool) async throws -> ClipEntry?
    /// `false` = "not a secret", `true` = "hide it", `nil` = back to detection.
    func clipboardSetSecret(id: UUID, secret: Bool?) async throws -> ClipEntry?
    func clipboardClear(includePinned: Bool) async throws -> Int
    func clipboardOpen(id: UUID, index: Int?) async throws
    func clipboardReveal(id: UUID, index: Int?) async throws
    func clipboardToSnippet(id: UUID) async throws
    func showClipboard() async throws
    func hideClipboard() async throws
    func showMainView(_ view: Route) async throws
    func setClipboardHotkey(accelerator: String) async throws -> Settings
    /// Clipboard media (`clipMediaUrl` / the `kzclip://` protocol): image bytes, or `nil`.
    func clipboardMedia(kind: ClipMediaKind, id: UUID, index: Int) async -> Data?
}

// MARK: - Defaults (the TypeScript default arguments)

extension Backend {
    public func showPermissions() async throws { try await showPermissions(kind: .accessibility) }
    public func renderPreview(content: String, snippetId: UUID?, formValues: [String: String]) async throws -> RenderResult {
        try await renderPreview(content: content, snippetId: snippetId, formValues: formValues, formatting: nil)
    }
    public func showMain() async throws { try await showMain(snippetId: nil) }
    public func clipboardCopy(id: UUID) async throws { try await clipboardCopy(id: id, plain: false) }
    public func clipboardPaste(id: UUID) async throws -> PasteOutcome { try await clipboardPaste(id: id, plain: false) }
    public func clipboardOpen(id: UUID) async throws { try await clipboardOpen(id: id, index: nil) }
    public func clipboardReveal(id: UUID) async throws { try await clipboardReveal(id: id, index: nil) }
    public func clipboardMedia(kind: ClipMediaKind, id: UUID) async -> Data? { await clipboardMedia(kind: kind, id: id, index: 0) }
}

/// A failed backend command, with the message the UI shows (the Tauri command's error string).
public struct BackendError: Error, Equatable, Sendable, CustomStringConvertible {
    public var message: String
    public init(_ message: String) { self.message = message }
    public var description: String { message }
}
