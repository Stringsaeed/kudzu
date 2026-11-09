import AppKit
import Foundation
import KudzuCore
import KudzuEngine
import KudzuServices

// Window, palette and form commands (`commands.rs` "Windows"), updates, sync and agents.
extension LiveBackend {
    public func showMain(snippetId: UUID?) async throws {
        shell?.hidePalette(restore: false)
        shell?.showMain()
        if let snippetId { emit(.navigate(.init(snippetId: snippetId))) }
    }

    public func showMainView(_ view: Route) async throws {
        shell?.hideClipboard(restore: false)
        shell?.showMain()
        emit(.navigate(.init(view: view)))
    }

    public func hidePalette() async throws {
        shell?.hidePalette(restore: true)
    }

    public func paletteInsert(snippetId: UUID) async throws {
        shell?.hidePalette(restore: false)
        engine.insertSnippet(id: snippetId, target: paletteTarget)
    }

    public func paletteCopy(snippetId: UUID) async throws {
        guard let result = engine.renderSnippetFormatted(id: snippetId) else { throw BackendError("Snippet not found") }
        copyRendered(result)
        shell?.hidePaletteAfterCopy()
    }

    public func paletteCreate(query: String) async throws {
        shell?.hidePalette(restore: false)
        shell?.showMain()
        let prefix = store.settings.defaultTriggerPrefix
        let looksLikeTrigger = !query.isEmpty && !query.contains(" ") && !prefix.isEmpty && query.hasPrefix(prefix)
        emit(.newSnippet(looksLikeTrigger ? .init(trigger: query) : .init(name: query)))
    }

    /// The status item's "New Snippet from Selection": copies the selection in the front app
    /// (⌘C, read, restore) off the main thread, then opens a new snippet with it.
    func newSnippetFromSelection() {
        Task { @MainActor [weak self] in
            let text = await Task.detached { System.copySelection() ?? "" }.value
            self?.shell?.showMain()
            self?.emit(.newSnippet(.init(content: text)))
        }
    }

    // MARK: Form

    public func pendingForm() async throws -> FormPayload? { pendingForm }

    public func submitForm(token: UInt64, values: [String: String]) async throws {
        let pending = pendingForm
        pendingForm = nil
        if let pending {
            store.withLock { $0.formMemory.record(values, for: pending.snippetId) }
            store.saveFormMemory()
        }
        shell?.hideForm()
        engine.setSuppressed(false)
        engine.submitForm(token: token, values: values)
    }

    public func cancelForm(token: UInt64) async throws {
        let pending = pendingForm
        pendingForm = nil
        shell?.hideForm()
        engine.setSuppressed(false)
        engine.cancelForm(token: token)
        if let target = pending?.targetApp { System.activateApp(target) }
    }

    public func resizeForm(height: Double) async throws {
        shell?.resizeForm(height: height)
    }

    // MARK: Updates

    /// The running app's version (`CFBundleShortVersionString`, i.e. package.json).
    public func appVersion() async throws -> String { identity.version }

    public func updateState() async throws -> UpdateState { updater.state }

    public func checkForUpdates() async throws -> UpdateState {
        await updater.check(quiet: false)
    }

    /// Installs the downloaded update and relaunches.
    public func restartToUpdate() async throws {
        do {
            try await updater.restartToUpdate { [weak self] in self?.shell?.quit() }
        } catch {
            throw backendError(error)
        }
    }

    // MARK: iCloud sync

    public func syncStatus() async throws -> SyncStatus { cloud.currentStatus }

    public func syncNow() async throws { cloud.recheck() }

    // MARK: AI agents

    public func agentSetup() async throws -> AgentSetup {
        AgentSetupInfo.make(identity: identity, running: agent.isRunning)
    }

    public func installAgentSkill() async throws -> String {
        do {
            return try AgentSetupInfo.installSkill()
        } catch {
            throw backendError(error)
        }
    }
}
