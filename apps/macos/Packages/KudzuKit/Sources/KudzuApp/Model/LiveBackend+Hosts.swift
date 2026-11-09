import Foundation
import KudzuCore
import KudzuEngine
import KudzuServices

/// The engine's callbacks into the app. Port of `host.rs`. Called on engine threads.
final class LiveEngineHost: EngineHost {
    private let backend: WeakBox<LiveBackend>

    init(backend: WeakBox<LiveBackend>) {
        self.backend = backend
    }

    func statusChanged(_ status: EngineStatus) {
        guard let b = backend.value else { return }
        b.bus.send(.status(status))
        Task { @MainActor in b.shell?.statusChanged(status) }
    }

    func expanded(_ event: ExpansionEvent) {
        guard let b = backend.value else { return }
        b.usage.enqueue {
            do { try await b.store.repository.recordUsage(snippetID: event.snippetId, deviceID: b.store.deviceID, charsSaved: event.charsSaved) }
            catch { b.log.error("saving usage failed: \(error.localizedDescription)") }
        }
    }

    func formRequested(_ request: FormRequest) {
        guard let b = backend.value else { return }
        let payload = b.store.withLock { c in
            FormPayload(
                token: request.token,
                snippetId: request.snippetId,
                snippetName: request.snippetName,
                fields: request.fields,
                initialValues: c.formMemory.initialValues(for: request.snippetId, fields: request.fields),
                content: b.store.library.snippets.first { $0.id == request.snippetId }?.content ?? "",
                targetApp: request.targetApp
            )
        }
        b.engine.setSuppressed(true)
        let caret = request.caret
        Task { @MainActor in
            b.pendingForm = payload
            b.shell?.showForm(caret: caret)
            b.emit(.formRequest(payload))
        }
    }
}

/// The agent socket's view of the app. Port of `agent.rs` `AppHost`.
struct AppAgentHost: Agent.Host, Sendable {
    let backend: WeakBox<LiveBackend>

    private func live() throws -> LiveBackend {
        guard let b = backend.value else { throw Agent.Failure(Agent.OFFLINE_MESSAGE) }
        return b
    }

    func access() throws -> Agent.Access {
        let s = try live().store.settings
        guard s.agentAccess else { throw Agent.Failure(Agent.OFFLINE_MESSAGE) }
        return Agent.Access(clipboard: s.agentClipboard && s.clipboardEnabled, write: s.agentWrite, triggerPrefix: s.defaultTriggerPrefix)
    }

    func library() throws -> Library { try live().store.library }

    func editLibrary(_ edit: (inout Library) throws -> [String: Any]) throws -> [String: Any] {
        throw Agent.Failure("Edits must execute in the repository request")
    }

    func clipboard() throws -> [ClipEntry] { try live().clipboard.entries }

    func clipboardText(_ id: UUID) throws -> String? { try live().clipboard.storedText(id) }

    /// `{{clipboard}}` in an agent's expansion reads the live pasteboard only when the user
    /// shares the clipboard with agents (the same gate as `list_clipboard`); otherwise it
    /// renders empty, as it does for a private item.
    func renderEnv() -> Agent.RenderEnv {
        var clipboard: String?
        if (try? access().clipboard) == true {
            let items = EngineClipboard.read()
            clipboard = Clipboard.isPrivate(items) ? nil : Clipboard.textOf(items)
        }
        return Agent.RenderEnv(now: Date(), locale: System.locale(), clipboard: clipboard, timeZone: .current)
    }
}
