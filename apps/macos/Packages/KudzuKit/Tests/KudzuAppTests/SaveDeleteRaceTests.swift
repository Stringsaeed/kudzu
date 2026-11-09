import Foundation
import KudzuCore
import KudzuDesign
import Testing
@testable import KudzuApp

/// A snippet deleted while its save is still on the way to the backend stays deleted
/// (`AppModel.saveSnippet` racing `deleteSnippets`, e.g. the editor's debounced save and ⌘⌫).
@MainActor @Suite struct SaveDeleteRaceTests {
    @Test func olderSnippetSaveReturnCannotOverwriteANewerLibraryEvent() async throws {
        let backend = HeldSaveBackend(late: .fail, holdAfterSave: true)
        let model = AppModel(backend: backend)
        await model.start()
        defer { model.stop() }
        var older = try #require(model.library.snippets.first)
        older.name = "Committed A"
        let saving = Task { try await model.saveSnippet(older) }
        await backend.waitUntilHolding()
        var newer = older
        newer.name = "Authoritative B"
        newer.content = "Later remote content"
        try await backend.saveImmediately(newer)
        #expect(await eventually { model.snippet(older.id)?.name == "Authoritative B" })
        await backend.releaseSave()
        try await saving.value
        #expect(model.snippet(older.id)?.name == "Authoritative B")
        #expect(model.snippet(older.id)?.content == "Later remote content")
    }

    @Test func olderGroupSaveReturnCannotOverwriteANewerLibraryEvent() async throws {
        let backend = HeldSaveBackend(late: .fail, holdAfterGroupSave: true)
        let model = AppModel(backend: backend)
        await model.start()
        defer { model.stop() }
        var older = try #require(model.library.groups.first)
        older.name = "Committed group A"
        let saving = Task { try await model.saveGroup(older) }
        await backend.waitUntilGroupSaveHeld()
        var newer = older
        newer.name = "Authoritative group B"
        newer.isEnabled = false
        try await backend.saveGroupImmediately(newer)
        #expect(await eventually { model.library.groups.first { $0.id == older.id }?.name == "Authoritative group B" })
        await backend.releaseGroupSave()
        try await saving.value
        #expect(model.library.groups.first { $0.id == older.id }?.name == "Authoritative group B")
        #expect(model.library.groups.first { $0.id == older.id }?.isEnabled == false)
    }

    @Test(arguments: [false, true]) func olderMoveCompletionCannotReplayOverNewerEvent(external: Bool) async throws {
        let backend = HeldSaveBackend(late: .fail, holdMove: true)
        let model = AppModel(backend: backend)
        await model.start()
        defer { model.stop() }
        let group = try #require(model.library.groups.first)
        let parentA = try await model.createGroup(name: "Parent A", parentId: nil)
        let parentB = try await model.createGroup(name: "Parent B", parentId: nil)
        #expect(await eventually { model.library.groups.contains { $0.id == parentB.id } })
        let older = Task { try await model.moveGroup(group.id, into: parentA.id) }
        await backend.waitUntilMoveHeld()
        if external {
            try await backend.moveImmediately(id: group.id, parent: parentB.id)
        } else {
            _ = try await model.moveGroup(group.id, into: parentB.id)
        }
        #expect(await eventually { model.library.groups.first { $0.id == group.id }?.parentID == parentB.id })
        await backend.releaseMove()
        #expect(try await older.value)
        let committed = try await backend.getState().library
        #expect(model.library == committed)
        #expect(model.library.groups.first { $0.id == group.id }?.parentID == parentB.id)
    }

    @Test func rejectedRenameRetainsItsNameInAnActionableRetry() async throws {
        let backend = HeldSaveBackend(late: .fail, failGroup: true)
        let model = AppModel(backend: backend)
        await model.start()
        defer { model.stop() }
        let group = try #require(model.library.groups.first)
        let actions = LibraryActions(model: model, focus: LibraryFocus())
        actions.renaming = group.id
        actions.commitRename(group.id, name: "Recovered group 日本")
        #expect(await eventually { ToastCenter.shared.toasts.contains { $0.title == "Couldn't rename group" && $0.description == "Rename disk full" } })
        let toast = try #require(ToastCenter.shared.toasts.first { $0.title == "Couldn't rename group" && $0.description == "Rename disk full" })
        let retry = try #require(toast.action)
        await backend.allowGroupSave()
        retry.action()
        #expect(await eventually { model.library.groups.first { $0.id == group.id }?.name == "Recovered group 日本" })
        actions.renaming = group.id
        actions.commitRename(group.id, name: nil)
        #expect(actions.renaming == nil)
        #expect(model.library.groups.first { $0.id == group.id }?.name == "Recovered group 日本")
        ToastCenter.shared.dismiss(toast.id)
    }

    @Test func rejectedSaveWhileDeleteIsPendingKeepsTheExactEditorDraft() async throws {
        let backend = HeldSaveBackend(late: .reject, failDelete: true, holdDelete: true)
        let model = AppModel(backend: backend)
        await model.start()
        defer { model.stop() }
        let original = try #require(model.library.snippets.first)
        let editor = model.editorSession
        await editor.select(original.id)
        editor.update { $0.content = "Recoverable 日本 draft" }
        let save = Task { await editor.flush() }
        await backend.waitUntilHolding()
        let deletion = Task { try await model.deleteSnippets([original.id]) }
        await backend.waitUntilDeleting()
        await backend.releaseSave()
        #expect(await save.value == false)
        #expect(editor.isDirty && !editor.didSave)
        #expect(editor.draft?.content == "Recoverable 日本 draft")
        await backend.releaseDelete()
        await #expect(throws: BackendError("Disk is full")) { try await deletion.value }
        #expect(editor.isDirty && editor.draft?.content == "Recoverable 日本 draft")
        await backend.allowSave()
        let retry = Task { await editor.flush() }
        await backend.waitUntilHolding()
        await backend.releaseSave()
        #expect(await retry.value)
        #expect(!editor.isDirty)
        #expect(try await backend.getState().library.snippets.first { $0.id == original.id }?.content == "Recoverable 日本 draft")
    }

    @Test func rejectedUndoCannotResurrectTheLateSaveAndRemainsRetryable() async throws {
        let backend = HeldSaveBackend(late: .upsert, failRestore: true)
        let model = AppModel(backend: backend)
        await model.start()
        defer { model.stop() }
        var edited = try #require(model.library.snippets.first)
        edited.content = "Snapshot for retry"
        let save = Task { try await model.saveSnippet(edited) }
        await backend.waitUntilHolding()
        let removed = try await model.deleteSnippets([edited.id])
        let undo = Task { try await model.restoreSnippets(removed) }
        await backend.releaseSave()
        try await save.value
        await #expect(throws: BackendError("Restore failed")) { try await undo.value }
        #expect(model.snippet(edited.id) == nil)
        #expect(try await !backend.getState().library.snippets.contains { $0.id == edited.id })
        await backend.allowRestore()
        try await model.restoreSnippets(removed)
        #expect(await eventually { model.snippet(edited.id)?.content == "Snapshot for retry" })
        #expect(try await backend.getState().library.snippets.filter { $0.id == edited.id }.count == 1)
    }

    @Test func rejectedSaveRestoresCommittedProjection() async throws {
        let backend = HeldSaveBackend(late: .reject)
        let model = AppModel(backend: backend)
        await model.start()
        let original = try #require(model.library.snippets.first)
        var edited = original
        edited.name = "Unsaved draft"
        let save = Task { try await model.saveSnippet(edited) }
        await backend.waitUntilHolding()
        await backend.releaseSave()
        await #expect(throws: BackendError("Disk is full")) { try await save.value }
        #expect(model.snippet(original.id) == original)
        #expect(model.selectedId == original.id)
        model.stop()
    }

    @Test func rejectedDeleteDoesNotLoseSelectionOrSuppressPendingSave() async throws {
        let backend = HeldSaveBackend(late: .fail, failDelete: true)
        let model = AppModel(backend: backend)
        await model.start()
        var edited = try #require(model.library.snippets.first)
        edited.name = "Saved after rejected deletion"
        let save = Task { try await model.saveSnippet(edited) }
        await backend.waitUntilHolding()
        await #expect(throws: BackendError("Disk is full")) { try await model.deleteSnippets([edited.id]) }
        #expect(model.selectedId == edited.id)
        #expect(model.snippet(edited.id) != nil)
        await backend.releaseSave()
        try await save.value
        #expect(model.snippet(edited.id)?.name == "Saved after rejected deletion")
        model.stop()
    }

    @Test func saveThatLandsAfterTheDeleteDoesNotResurrectTheSnippet() async throws {
        let backend = HeldSaveBackend(late: .upsert)
        let (model, id) = try await raceDeleteAgainstSave(backend)
        #expect(model.snippet(id) == nil)
        #expect(try await !backend.getState().library.snippets.contains { $0.id == id })
        #expect(await eventually { model.snippet(id) == nil && model.library.snippets.count == 14 })
    }

    @Test func saveRejectedBecauseOfTheDeleteIsNotAnError() async throws {
        // `LiveBackend.saveSnippet` throws "That snippet no longer exists." once the delete ran.
        let backend = HeldSaveBackend(late: .fail)
        let (model, id) = try await raceDeleteAgainstSave(backend)
        #expect(model.snippet(id) == nil)
        #expect(try await !backend.getState().library.snippets.contains { $0.id == id })
    }

    @Test func undoDuringTheSaveKeepsTheSnippet() async throws {
        let backend = HeldSaveBackend(late: .upsert)
        let model = AppModel(backend: backend)
        await model.start()
        var edited = try #require(model.library.snippets.first)
        edited.name = "Edited mid-delete"
        let save = Task { try await model.saveSnippet(edited) }
        await backend.waitUntilHolding()
        let removed = try await model.deleteSnippets([edited.id])
        let undo = Task { try await model.restoreSnippets(removed) }
        await backend.releaseSave()
        try await save.value
        try await undo.value
        #expect(await eventually { model.snippet(edited.id)?.name == "Edited mid-delete" })
        #expect(try await backend.getState().library.snippets.contains { $0.id == edited.id })
    }

    /// Starts a save, deletes the snippet while the backend holds it, then lets the save land.
    private func raceDeleteAgainstSave(_ backend: HeldSaveBackend) async throws -> (AppModel, UUID) {
        let model = AppModel(backend: backend)
        await model.start()
        var edited = try #require(model.library.snippets.first)
        edited.name = "Edited mid-delete"
        let save = Task { try await model.saveSnippet(edited) }
        await backend.waitUntilHolding()
        try await model.deleteSnippets([edited.id])
        await backend.releaseSave()
        try await save.value
        return (model, edited.id)
    }

    private func eventually(_ condition: @MainActor () -> Bool) async -> Bool {
        for _ in 0..<200 {
            if condition() { return true }
            try? await Task.sleep(for: .milliseconds(10))
        }
        return condition()
    }
}

/// `MockBackend`, except that `saveSnippet` waits for ``releaseSave()``. When the snippet was
/// deleted meanwhile, the save either re-creates it (`.upsert`, a backend that writes the record
/// whole) or fails as `LiveBackend` does (`.fail`).
private actor HeldSaveBackend: Backend {
    enum Late { case upsert, fail, reject }

    let base: any Backend = MockBackend()
    let late: Late
    let failDelete: Bool
    let holdDelete: Bool
    private var failRestore: Bool
    private var rejectsSave: Bool
    private var failGroup: Bool
    private var holdsNextMove: Bool
    private let holdAfterSave: Bool
    private let holdAfterGroupSave: Bool
    private var groupSaveRelease: CheckedContinuation<Void, Never>?
    private var groupSaveHolders: [CheckedContinuation<Void, Never>] = []
    private var moveRelease: CheckedContinuation<Void, Never>?
    private var moveHolders: [CheckedContinuation<Void, Never>] = []
    private var release: CheckedContinuation<Void, Never>?
    private var holders: [CheckedContinuation<Void, Never>] = []
    private var deletion: CheckedContinuation<Void, Never>?
    private var deleteHolders: [CheckedContinuation<Void, Never>] = []

    init(late: Late, failDelete: Bool = false, holdDelete: Bool = false, failRestore: Bool = false, failGroup: Bool = false, holdMove: Bool = false, holdAfterSave: Bool = false, holdAfterGroupSave: Bool = false) {
        self.late = late
        self.failDelete = failDelete
        self.holdDelete = holdDelete
        self.failRestore = failRestore
        self.failGroup = failGroup
        self.holdsNextMove = holdMove
        self.holdAfterSave = holdAfterSave
        self.holdAfterGroupSave = holdAfterGroupSave
        if case .reject = late { rejectsSave = true } else { rejectsSave = false }
    }

    nonisolated var events: AsyncStream<BackendEvent> { base.events }

    func waitUntilHolding() async {
        if release != nil { return }
        await withCheckedContinuation { holders.append($0) }
    }

    func releaseSave() {
        release?.resume()
        release = nil
    }

    func allowSave() { rejectsSave = false }
    func allowRestore() { failRestore = false }
    func allowGroupSave() { failGroup = false }

    func waitUntilDeleting() async {
        if deletion != nil { return }
        await withCheckedContinuation { deleteHolders.append($0) }
    }

    func releaseDelete() {
        deletion?.resume()
        deletion = nil
    }

    func saveSnippet(_ snippet: Snippet) async throws -> Snippet {
        if holdAfterSave {
            let saved = try await base.saveSnippet(snippet)
            await holdSave()
            return saved
        }
        await holdSave()
        if rejectsSave { throw BackendError("Disk is full") }
        if try await base.getState().library.snippets.contains(where: { $0.id == snippet.id }) {
            return try await base.saveSnippet(snippet)
        }
        switch late {
        case .upsert:
            try await base.restoreSnippets([snippet])
            return snippet
        case .fail, .reject:
            throw BackendError("That snippet no longer exists.")
        }
    }

    private func holdSave() async {
        await withCheckedContinuation { c in
            release = c
            holders.forEach { $0.resume() }
            holders = []
        }
    }

    func saveImmediately(_ snippet: Snippet) async throws { _ = try await base.saveSnippet(snippet) }

    func getState() async throws -> AppState { try await base.getState() }
    func createSnippet(draft: SnippetDraft) async throws -> Snippet { try await base.createSnippet(draft: draft) }
    func deleteSnippets(ids: [UUID]) async throws {
        if holdDelete {
            await withCheckedContinuation { c in
                deletion = c
                deleteHolders.forEach { $0.resume() }
                deleteHolders = []
            }
        }
        if failDelete { throw BackendError("Disk is full") }
        try await base.deleteSnippets(ids: ids)
    }
    func restoreSnippets(_ snippets: [Snippet]) async throws {
        if failRestore { throw BackendError("Restore failed") }
        try await base.restoreSnippets(snippets)
    }
    func duplicateSnippet(id: UUID) async throws -> Snippet { try await base.duplicateSnippet(id: id) }
    func moveSnippets(ids: [UUID], groupId: UUID?) async throws { try await base.moveSnippets(ids: ids, groupId: groupId) }
    func saveGroup(_ group: Group) async throws -> Group {
        if failGroup { throw BackendError("Rename disk full") }
        let saved = try await base.saveGroup(group)
        if holdAfterGroupSave {
            await withCheckedContinuation { continuation in
                groupSaveRelease = continuation
                groupSaveHolders.forEach { $0.resume() }
                groupSaveHolders = []
            }
        }
        return saved
    }

    func saveGroupImmediately(_ group: Group) async throws { _ = try await base.saveGroup(group) }
    func waitUntilGroupSaveHeld() async {
        if groupSaveRelease != nil { return }
        await withCheckedContinuation { groupSaveHolders.append($0) }
    }
    func releaseGroupSave() { groupSaveRelease?.resume(); groupSaveRelease = nil }
    func createGroup(name: String, parentId: UUID?) async throws -> Group { try await base.createGroup(name: name, parentId: parentId) }
    func deleteGroup(id: UUID, deleteSnippets: Bool) async throws { try await base.deleteGroup(id: id, deleteSnippets: deleteSnippets) }
    func moveGroup(id: UUID, parentId: UUID?, index: Int) async throws {
        try await base.moveGroup(id: id, parentId: parentId, index: index)
        if holdsNextMove {
            holdsNextMove = false
            await withCheckedContinuation { continuation in
                moveRelease = continuation
                moveHolders.forEach { $0.resume() }
                moveHolders = []
            }
        }
    }

    func waitUntilMoveHeld() async {
        if moveRelease != nil { return }
        await withCheckedContinuation { moveHolders.append($0) }
    }

    func releaseMove() {
        moveRelease?.resume()
        moveRelease = nil
    }

    func moveImmediately(id: UUID, parent: UUID?) async throws {
        try await base.moveGroup(id: id, parentId: parent, index: 0)
    }
    func findConflicts() async throws -> [TriggerConflict] { try await base.findConflicts() }
    func showPermissions(kind: PermissionKind) async throws { try await base.showPermissions(kind: kind) }
    func permissionContext() async throws -> PermissionContext { try await base.permissionContext() }
    func installPermissionIcon(bounds: ScreenRect) async throws -> Bool { try await base.installPermissionIcon(bounds: bounds) }
    func beginPermission(kind: PermissionKind) async throws { try await base.beginPermission(kind: kind) }
    func stopPermissionGuide() async throws { try await base.stopPermissionGuide() }
    func restartPermissionApp() async throws { try await base.restartPermissionApp() }
    func closePermissions() async throws { try await base.closePermissions() }
    func revealPermissionApp() async throws { try await base.revealPermissionApp() }
    func updateSettings(_ patch: SettingsPatch) async throws -> Settings { try await base.updateSettings(patch) }
    func setPaused(_ paused: Bool) async throws -> EngineStatus { try await base.setPaused(paused) }
    func requestPermission(kind: PermissionKind) async throws { try await base.requestPermission(kind: kind) }
    func openSystemSettings(pane: SystemSettingsPane) async throws { try await base.openSystemSettings(pane: pane) }
    func engineStatus() async throws -> EngineStatus { try await base.engineStatus() }
    func renderPreview(content: String, snippetId: UUID?, formValues: [String: String], formatting: Formatting?) async throws -> RenderResult { try await base.renderPreview(content: content, snippetId: snippetId, formValues: formValues, formatting: formatting) }
    func copyPreview(content: String, snippetId: UUID?, formatting: Formatting?) async throws { try await base.copyPreview(content: content, snippetId: snippetId, formatting: formatting) }
    func listApps() async throws -> [AppInfo] { try await base.listApps() }
    func appIcon(path: String) async throws -> Data? { try await base.appIcon(path: path) }
    func detectEspanso() async throws -> [EspansoDetection] { try await base.detectEspanso() }
    func importEspanso(path: String) async throws -> ImportSummary { try await base.importEspanso(path: path) }
    func importFile(path: String) async throws -> ImportSummary { try await base.importFile(path: path) }
    func exportJson(path: String) async throws { try await base.exportJson(path: path) }
    func exportEspanso(dir: String) async throws -> Int { try await base.exportEspanso(dir: dir) }
    func listBackups() async throws -> [BackupInfo] { try await base.listBackups() }
    func backupNow() async throws -> BackupInfo { try await base.backupNow() }
    func restoreBackup(path: String) async throws { try await base.restoreBackup(path: path) }
    func revealDataFolder() async throws { try await base.revealDataFolder() }
    func showMain(snippetId: UUID?) async throws { try await base.showMain(snippetId: snippetId) }
    func hidePalette() async throws { try await base.hidePalette() }
    func paletteInsert(snippetId: UUID) async throws { try await base.paletteInsert(snippetId: snippetId) }
    func paletteCopy(snippetId: UUID) async throws { try await base.paletteCopy(snippetId: snippetId) }
    func paletteCreate(query: String) async throws { try await base.paletteCreate(query: query) }
    func setPaletteHotkey(accelerator: String) async throws -> Settings { try await base.setPaletteHotkey(accelerator: accelerator) }
    func pendingForm() async throws -> FormPayload? { try await base.pendingForm() }
    func submitForm(token: UInt64, values: [String: String]) async throws { try await base.submitForm(token: token, values: values) }
    func cancelForm(token: UInt64) async throws { try await base.cancelForm(token: token) }
    func resizeForm(height: Double) async throws { try await base.resizeForm(height: height) }
    func appVersion() async throws -> String { try await base.appVersion() }
    func updateState() async throws -> UpdateState { try await base.updateState() }
    func checkForUpdates() async throws -> UpdateState { try await base.checkForUpdates() }
    func restartToUpdate() async throws { try await base.restartToUpdate() }
    func syncStatus() async throws -> SyncStatus { try await base.syncStatus() }
    func syncNow() async throws { try await base.syncNow() }
    func agentSetup() async throws -> AgentSetup { try await base.agentSetup() }
    func installAgentSkill() async throws -> String { try await base.installAgentSkill() }
    func clipboardList() async throws -> ClipboardInfo { try await base.clipboardList() }
    func clipboardText(id: UUID) async throws -> String? { try await base.clipboardText(id: id) }
    func clipboardCopy(id: UUID, plain: Bool) async throws { try await base.clipboardCopy(id: id, plain: plain) }
    func clipboardPaste(id: UUID, plain: Bool) async throws -> PasteOutcome { try await base.clipboardPaste(id: id, plain: plain) }
    func clipboardDelete(ids: [UUID]) async throws -> [ClipEntry] { try await base.clipboardDelete(ids: ids) }
    func clipboardRestore(_ entries: [ClipEntry]) async throws { try await base.clipboardRestore(entries) }
    func clipboardPin(id: UUID, pinned: Bool) async throws -> ClipEntry? { try await base.clipboardPin(id: id, pinned: pinned) }
    func clipboardSetSecret(id: UUID, secret: Bool?) async throws -> ClipEntry? { try await base.clipboardSetSecret(id: id, secret: secret) }
    func clipboardClear(includePinned: Bool) async throws -> Int { try await base.clipboardClear(includePinned: includePinned) }
    func clipboardOpen(id: UUID, index: Int?) async throws { try await base.clipboardOpen(id: id, index: index) }
    func clipboardReveal(id: UUID, index: Int?) async throws { try await base.clipboardReveal(id: id, index: index) }
    func clipboardToSnippet(id: UUID) async throws { try await base.clipboardToSnippet(id: id) }
    func showClipboard() async throws { try await base.showClipboard() }
    func hideClipboard() async throws { try await base.hideClipboard() }
    func showMainView(_ view: Route) async throws { try await base.showMainView(view) }
    func setClipboardHotkey(accelerator: String) async throws -> Settings { try await base.setClipboardHotkey(accelerator: accelerator) }
    func clipboardMedia(kind: ClipMediaKind, id: UUID, index: Int) async -> Data? { await base.clipboardMedia(kind: kind, id: id, index: index) }
}
