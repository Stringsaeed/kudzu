import Foundation
import KudzuCore
import Observation

/// The editor owns its draft until a successful save or an explicit discard. Saves use the
/// model's existing path and run serially, so an older response cannot win over a newer edit.
@MainActor @Observable
public final class EditorSession {
    public private(set) var draft: Snippet?
    public private(set) var isDirty = false
    public private(set) var isSaving = false
    public private(set) var didSave = false
    public private(set) var error: String?
    public private(set) var preview: RenderResult?
    @ObservationIgnored private weak var model: AppModel?
    @ObservationIgnored private var debounce: Task<Void, Never>?
    @ObservationIgnored private var saving: Task<Bool, Never>?
    @ObservationIgnored private var revision = 0
    @ObservationIgnored private var navigation = 0
    @ObservationIgnored private var baseline: Snippet?
    @ObservationIgnored private let persist: @MainActor (Snippet) async throws -> Void
    /// Runs at the start of every save and navigation. The editor uses it to commit IME text into
    /// the draft before that draft is written or replaced. Nil when no editor is on screen.
    @ObservationIgnored public var beforeFlush: (() -> Void)?
    @ObservationIgnored public var isComposing: (() -> Bool)?

    init(model: AppModel, persist: (@MainActor (Snippet) async throws -> Void)? = nil) {
        self.model = model
        self.persist = persist ?? { [weak model] snippet in try await model?.saveSnippet(snippet) }
    }

    public func update(_ edit: (inout Snippet) -> Void) {
        guard var next = draft else { return }
        edit(&next)
        draft = next
        revision += 1
        isDirty = true
        didSave = false
        error = nil
        debounce?.cancel()
        debounce = Task { [weak self] in
            do { try await Task.sleep(for: .milliseconds(350)) } catch { return }
            guard !Task.isCancelled, let self, self.isComposing?() != true else { return }
            _ = await self.flush(commitComposition: false)
        }
    }

    /// Called for a selection request from any host. A failed save keeps the old selection.
    public func select(_ id: UUID?) async {
        guard let model else { return }
        navigation += 1
        let request = navigation
        // Commit marked text before deciding whether the draft is dirty, or leaving this snippet
        // drops an IME composition that was never published.
        beforeFlush?()
        if let id, draft?.id == id { model.selectedId = id; reconcile(); return }
        if let old = draft, old.id != id, isDirty || isSaving, model.snippet(old.id) != nil {
            model.selectedId = old.id
            guard await flush(), request == navigation else { return }
        }
        guard request == navigation else { return }
        if draft?.id != id { preview = nil }
        model.selectedId = id
        draft = id.flatMap(model.snippet)
        baseline = draft
        isDirty = false
        error = nil
        didSave = false
    }

    /// External changes refresh a clean editor. A removed record invalidates pending autosave:
    /// disappear/blur must never send a deleted draft back to an upserting backend.
    public func reconcile() {
        guard let model, let current = draft else { return }
        guard let latest = model.snippet(current.id) else {
            debounce?.cancel()
            revision += 1
            draft = nil
            isDirty = false
            error = nil
            return
        }
        if !isDirty, !isSaving { draft = latest; baseline = latest }
    }

    public func refreshPreview() async throws {
        guard let model, let current = draft else { return }
        let requestedRevision = revision
        let result = try await model.backend.renderPreview(content: current.content, snippetId: current.id, formValues: [:], formatting: current.formatting)
        guard !Task.isCancelled, draft?.id == current.id, revision == requestedRevision else { return }
        preview = result
    }

    @discardableResult
    public func flush() async -> Bool {
        await flush(commitComposition: true)
    }

    private func flush(commitComposition: Bool) async -> Bool {
        if commitComposition { beforeFlush?() }
        debounce?.cancel()
        if let saving { return await saving.value }
        guard isDirty else { return true }
        let task = Task { [weak self] in await self?.savePending() ?? false }
        saving = task
        let result = await task.value
        saving = nil
        return result
    }

    private func savePending() async -> Bool {
        guard let model else { return false }
        isSaving = true
        defer { isSaving = false }
        while isDirty, let current = draft {
            guard model.snippet(current.id) != nil else { reconcile(); return true }
            let sent = revision
            do {
                try await persist(current)
                baseline = model.snippet(current.id)
                guard draft?.id == current.id else { return true }
                if revision == sent {
                    draft = model.snippet(current.id)
                    isDirty = false
                    didSave = true
                    error = nil
                }
            } catch {
                self.error = "Couldn't save: \(AppModel.errorMessage(error))"
                model.selectedId = current.id
                return false
            }
        }
        return true
    }

    public func discard() {
        debounce?.cancel()
        // An in-flight write cannot be undone by discarding. The UI disables discard until it ends.
        guard !isSaving else { return }
        revision += 1
        if let current = draft, model?.snippet(current.id) != nil {
            draft = baseline
            if let baseline { model?.upsertLocal(baseline) }
        } else { draft = nil }
        isDirty = false
        error = nil
        didSave = false
    }
}
