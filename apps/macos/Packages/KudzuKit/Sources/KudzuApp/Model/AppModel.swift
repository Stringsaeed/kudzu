import Foundation
import KudzuCore
import Observation

/// Main-window state: the port of `lib/store.ts`, the clipboard store of `lib/clipboard.ts` and
/// the update/sync hooks of `lib/updates.ts`. The backend owns the library; this model mirrors
/// it (kept fresh by backend events) and holds view state. Mutations are optimistic, then
/// confirmed by the backend's answer.
///
/// Features extend it only in their own `Model/AppModel+<Feature>.swift`.
@MainActor @Observable
public final class AppModel {
    public let backend: any Backend
    /// Kudzu Dev (separate data folder, different hotkeys and MCP server name).
    public let isDev: Bool

    // MARK: Backend mirror

    public private(set) var ready = false
    public var platform: Platform = .web
    public var library: Library = .empty
    public var settings: Settings = AppModel.initialSettings
    public var status: EngineStatus = .starting
    /// Usage summed across devices, by snippet.
    public var usage: [UUID: SnippetUsage] = [:]
    public var conflicts: [TriggerConflict] = []
    /// `useUpdateState`: seeded from the backend, kept current by `update` events.
    public var updateState: UpdateState = .idle
    /// `useSyncStatus`: seeded from the backend, kept current by `sync` events.
    public var syncStatus: SyncStatus = .starting
    /// The clipboard history (`useClipboard`).
    public var clipboard = ClipboardState()

    // MARK: View state

    public var view: Route = .all
    public var selectedId: UUID?
    public var filter = ""
    public var commandOpen = false
    public var shortcutsOpen = false
    /// Snippet id whose name field should take focus on next render (new snippets).
    public var focusNameOf: UUID?

    // MARK: Window-level events

    /// The form the form panel should show (`form-request`).
    public var formRequest: FormPayload?
    /// The app the palette opened over (`palette-show`).
    public var paletteTarget: AppInfo?
    /// The app the clipboard picker opened over (`clipboard-show`).
    public var clipboardTarget: AppInfo?
    /// The permission the companion window guides (`permission-show`).
    public var permissionKind: PermissionKind = .accessibility

    @ObservationIgnored private var eventTask: Task<Void, Never>?
    @ObservationIgnored private var conflictTask: Task<Void, Never>?
    @ObservationIgnored let clipTextCache = ClipTextCache()
    @ObservationIgnored private let initialView: Route?
    /// Snippet saves awaiting the backend, by id, and the ids deleted while one was in flight
    /// (``saveSnippet(_:)`` must not write those back).
    @ObservationIgnored private var savesInFlight: [UUID: Int] = [:]
    @ObservationIgnored private var deletedMidSave: Set<UUID> = []
    @ObservationIgnored private var saveWaiters: [UUID: [CheckedContinuation<Void, Never>]] = [:]
    @ObservationIgnored private var libraryEventGeneration: UInt64 = 0

    /// - Parameter initialView: the browser preview's `?view=` deep link.
    public init(backend: any Backend, isDev: Bool = false, initialView: Route? = nil) {
        self.backend = backend
        self.isDev = isDev
        self.initialView = initialView
    }

    /// The settings shown before the backend answers (`lib/store.ts`'s initial state).
    public static let initialSettings: Settings = {
        var s = Settings.default
        s.deniedBundleIDs = []
        s.appInsertions = [:]
        s.onboardingCompleted = true
        s.clipboardIgnoredApps = []
        return s
    }()

    // MARK: Lifecycle

    /// Subscribes to backend events, then loads the state (`init` of every store). Call once;
    /// ``stop()`` unsubscribes.
    public func start() async {
        let events = backend.events
        eventTask?.cancel()
        eventTask = Task { [weak self] in
            for await event in events {
                guard let self else { return }
                self.handle(event)
            }
        }
        async let clip: Void = refreshClipboard()
        async let updates: Void = loadUpdateAndSync()
        do {
            let initialLibraryGeneration = libraryEventGeneration
            let s = try await backend.getState()
            ready = true
            platform = s.platform
            if libraryEventGeneration == initialLibraryGeneration { library = s.library }
            settings = s.settings
            status = s.status
            usage = Stats.usageById(library)
            selectedId = selectedId ?? s.library.snippets.first?.id
            if let initialView { view = initialView }
            refreshConflicts()
        } catch {
            // The TypeScript store left the window in its initial state too.
        }
        _ = await (clip, updates)
    }

    /// Stops listening to backend events.
    public func stop() {
        eventTask?.cancel()
        eventTask = nil
        conflictTask?.cancel()
    }

    private func loadUpdateAndSync() async {
        if let s = try? await backend.updateState() { updateState = s }
        if let s = try? await backend.syncStatus() { syncStatus = s }
    }

    // MARK: Events

    /// Applies one backend event (exposed for the shell and tests).
    public func handle(_ event: BackendEvent) {
        switch event {
        case let .library(library):
            libraryEventGeneration &+= 1
            self.library = library
            usage = Stats.usageById(library)
            refreshConflicts()
        case let .settings(settings): self.settings = settings
        case let .status(status): self.status = status
        case let .update(state): updateState = state
        case let .sync(status): syncStatus = status
        case let .clipboard(event): applyClipEvent(event)
        case let .navigate(p):
            if let v = p.view { view = v }
            if let id = p.snippetId {
                let s = library.snippets.first { $0.id == id }
                selectedId = id
                view = s?.groupID.map { .group(id: $0) } ?? .all
                filter = ""
            }
        case let .newSnippet(p):
            let prefix = settings.defaultTriggerPrefix
            let triggers = p.trigger.map { [$0.hasPrefix(prefix) ? $0 : prefix + $0] } ?? []
            Task { _ = try? await newSnippet(SnippetDraft(name: p.name ?? "", triggers: triggers, content: p.content ?? "")) }
        case let .formRequest(form): formRequest = form
        case let .paletteShow(target): paletteTarget = target
        case let .clipboardShow(target, canPaste):
            clipboardTarget = target
            if let canPaste { clipboard.canPaste = canPaste }
        case let .permissionShow(kind): permissionKind = kind
        case .command: break
        }
    }

    // MARK: View state

    public func setView(_ view: Route) { self.view = view }
    public func select(_ id: UUID?) { selectedId = id }
    public func setFilter(_ q: String) { filter = q }
    public func consumeFocusName() { focusNameOf = nil }

    /// The snippet with `id`, if loaded.
    public func snippet(_ id: UUID) -> Snippet? { library.snippets.first { $0.id == id } }

    /// Replaces a snippet in place without telling the backend.
    public func upsertLocal(_ snippet: Snippet) {
        library.snippets = library.snippets.map { $0.id == snippet.id ? snippet : $0 }
    }

    // MARK: Snippets

    /// Creates a snippet in the current group (and as a favourite in Favorites), selects it and
    /// focuses its name.
    @discardableResult
    public func newSnippet(_ draft: SnippetDraft = SnippetDraft()) async throws -> Snippet {
        let current = view
        var d = draft
        if case let .group(id) = current, d.groupID == nil { d.groupID = id }
        if d.isFavorite == nil { d.isFavorite = current == .favorites }
        let snippet = try await backend.createSnippet(draft: d)
        if !library.snippets.contains(where: { $0.id == snippet.id }) { library.snippets.insert(snippet, at: 0) }
        selectedId = snippet.id
        focusNameOf = snippet.id
        filter = ""
        switch current {
        case .insights, .settings, .clipboard, .disabled, .recent: view = .all
        default: view = current
        }
        return snippet
    }

    public func saveSnippet(_ snippet: Snippet) async throws {
        let id = snippet.id
        let eventGeneration = libraryEventGeneration
        upsertLocal(snippet)
        savesInFlight[id, default: 0] += 1
        defer {
            savesInFlight[id, default: 1] -= 1
            if savesInFlight[id] == 0 {
                savesInFlight[id] = nil
                deletedMidSave.remove(id)
                let waiters = saveWaiters.removeValue(forKey: id) ?? []
                for waiter in waiters { waiter.resume() }
            }
        }
        let saved: Snippet
        do {
            saved = try await backend.saveSnippet(snippet)
        } catch where deletedMidSave.contains(id) {
            // The delete reached the backend first ("That snippet no longer exists."): the
            // snippet is meant to be gone, so this is not an error for the editor to show.
            return
        } catch {
            await reconcileLibraryAfterFailure()
            throw error
        }
        if deletedMidSave.contains(id) {
            // A committed delete wins even when the late save's event already reintroduced the
            // record into the projection. Undo waits for this cleanup before restoring it.
            try await backend.deleteSnippets(ids: [id])
            library.snippets.removeAll { $0.id == id }
            return
        }
        guard library.snippets.contains(where: { $0.id == id }) else { return }
        if eventGeneration == libraryEventGeneration { upsertLocal(saved) }
    }

    /// Removes snippets (selecting the neighbour of a deleted selection) and returns them for undo.
    @discardableResult
    public func deleteSnippets(_ ids: [UUID]) async throws -> [Snippet] {
        let doomed = Set(ids)
        let removed = library.snippets.filter { doomed.contains($0.id) }
        let remaining = library.snippets.filter { !doomed.contains($0.id) }
        var next = selectedId
        if let selected = selectedId, doomed.contains(selected) {
            let i = library.snippets.firstIndex { $0.id == selected } ?? 0
            next = remaining.isEmpty ? nil : remaining[min(i, remaining.count - 1)].id
        }
        let previousSelection = selectedId
        do {
            try await backend.deleteSnippets(ids: ids)
        } catch {
            await reconcileLibraryAfterFailure()
            if selectedId == next || selectedId == previousSelection { selectedId = previousSelection }
            throw error
        }
        for id in doomed where savesInFlight[id] != nil { deletedMidSave.insert(id) }
        library.snippets.removeAll { doomed.contains($0.id) }
        if selectedId == previousSelection { selectedId = next }
        return removed
    }

    public func restoreSnippets(_ snippets: [Snippet]) async throws {
        // Finish any save predating the delete (including late-upsert cleanup) before Undo.
        // A failed restore must never clear a committed deletion's protection prematurely.
        for id in snippets.map(\.id) where savesInFlight[id] != nil {
            await withCheckedContinuation { saveWaiters[id, default: []].append($0) }
        }
        try await backend.restoreSnippets(snippets)
        deletedMidSave.subtract(snippets.map(\.id))
        selectedId = snippets.first?.id ?? selectedId
    }

    public func duplicate(_ id: UUID) async throws {
        let copy = try await backend.duplicateSnippet(id: id)
        if !library.snippets.contains(where: { $0.id == copy.id }) { library.snippets.insert(copy, at: 0) }
        selectedId = copy.id
        focusNameOf = copy.id
    }

    public func toggleFavorite(_ id: UUID) async throws {
        guard var s = snippet(id) else { return }
        s.isFavorite.toggle()
        try await saveSnippet(s)
    }

    public func toggleEnabled(_ id: UUID) async throws {
        guard var s = snippet(id) else { return }
        s.isEnabled.toggle()
        try await saveSnippet(s)
    }

    public func moveSnippets(_ ids: [UUID], to groupId: UUID?) async throws {
        try await backend.moveSnippets(ids: ids, groupId: groupId)
        let moving = Set(ids)
        for i in library.snippets.indices where moving.contains(library.snippets[i].id) {
            library.snippets[i].groupID = groupId
        }
    }

    // MARK: Groups

    @discardableResult
    public func createGroup(name: String, parentId: UUID?) async throws -> Group {
        let group = try await backend.createGroup(name: name, parentId: parentId)
        if !library.groups.contains(where: { $0.id == group.id }) { library.groups.append(group) }
        view = .group(id: group.id)
        return group
    }

    public func saveGroup(_ group: Group) async throws {
        let eventGeneration = libraryEventGeneration
        let saved = try await backend.saveGroup(group)
        if eventGeneration == libraryEventGeneration {
            library.groups = library.groups.map { $0.id == saved.id ? saved : $0 }
        }
    }

    public func deleteGroup(_ id: UUID, deleteSnippets: Bool) async throws {
        try await backend.deleteGroup(id: id, deleteSnippets: deleteSnippets)
        if view == .group(id: id) { view = .all }
    }

    // MARK: Settings & engine

    public func updateSettings(_ patch: SettingsPatch) async throws {
        settings = try await backend.updateSettings(patch)
    }

    public func setPaused(_ paused: Bool) async throws {
        status = try await backend.setPaused(paused)
        settings.paused = status.paused
    }

    /// Restore the committed projection after a rejected optimistic save. EditorSession keeps
    /// its draft independently, so retrying does not depend on pretending the save succeeded.
    private func reconcileLibraryAfterFailure() async {
        let eventGeneration = libraryEventGeneration
        guard let state = try? await backend.getState() else { return }
        guard eventGeneration == libraryEventGeneration else { return }
        library = state.library
        usage = Stats.usageById(state.library)
        refreshConflicts()
    }

    /// Re-checks trigger conflicts 250 ms after the last library change.
    public func refreshConflicts() {
        conflictTask?.cancel()
        conflictTask = Task { [weak self, backend] in
            try? await Task.sleep(for: .milliseconds(250))
            guard !Task.isCancelled, let conflicts = try? await backend.findConflicts() else { return }
            self?.conflicts = conflicts
        }
    }
}
