import Foundation
import KudzuCore

/// In-memory stand-in for the real backend, for SwiftUI previews and tests. Port of
/// `lib/mock.ts`: same sample data, same fake update/sync/clipboard behaviour. The browser
/// preview's query flags (`?update`, `?nosync`, `?espanso`, …) are ``Options``.
///
/// Differences from the TypeScript mock: nothing persists (the browser kept the library in
/// `localStorage`), copies are recorded through ``recordCopy(_:)`` instead of a DOM listener,
/// and `clipboardCopy`/`clipboardPaste` never touch the real pasteboard.
public actor MockBackend: Backend {
    /// The preview flags of `lib/mock.ts` / `lib/store.ts`.
    public struct Options: Sendable {
        /// `?light`: start with the light theme.
        public var light = false
        /// `?onboarding`: start with onboarding not completed.
        public var onboarding = false
        /// `?update`: a downloaded 0.2.0 is ready to install.
        public var update = false
        /// `?nosync`: iCloud is unavailable (unsigned build).
        public var noSync = false
        /// `?espanso`: two Espanso sources are detected; `.retry` makes the second fail once.
        public var espanso: EspansoPreview = .none
        /// `?finish=retry`: the first `onboardingCompleted` save fails.
        public var finishRetry = false
        /// `?demo`: `pendingForm` answers with the first form snippet.
        public var demoForm = false
        /// `?emptyclips`: start with an empty clipboard history.
        public var emptyClips = false
        /// `?clips=N`: N generated copies on top of the samples.
        public var extraClips = 0
        /// `/permissions` route: start with the permission state of this preview.
        public var permission: PermissionPreview?
        /// Which permission the companion guides (`?kind=`).
        public var permissionKind: PermissionKind = .accessibility
        /// `get_state`'s platform (`web` in the browser preview).
        public var platform: Platform = .web
        /// Delay of `checkForUpdates` (700 ms in the preview).
        public var checkDelay: Duration = .milliseconds(700)

        public init() {}
    }

    public enum EspansoPreview: Sendable { case none, present, retry }

    /// The permission states the preview can show (`previewPermission`).
    public enum PermissionPreview: String, Sendable {
        case missing, revoked, allowed, restart, paused, secure
    }

    // MARK: State

    // Internal (not private) so the command extensions in sibling files can reach them.
    var state: AppState
    var clips: [ClipEntry]
    var artwork: MockClips.Artwork
    var media: [UUID: Data] = [:]
    var importPreviewFailed = false
    var importedPreviews = Set<String>()
    var finishPreviewFailed = false
    var skillInstalled = false
    let options: Options
    let bus = EventBroadcaster()

    public init(options: Options = Options()) {
        self.options = options
        var settings = MockSeed.settings
        if options.light { settings.theme = .light }
        if options.onboarding { settings.onboardingCompleted = false }
        state = AppState(library: MockSeed.library(), settings: settings, status: Self.status(paused: settings.paused), platform: options.platform)
        let seeded = options.emptyClips ? (entries: [], artwork: [:]) : MockClips.seed(extra: options.extraClips)
        clips = seeded.entries
        artwork = seeded.artwork
        if let permission = options.permission { state.status = Self.permissionStatus(permission) }
    }

    public nonisolated var events: AsyncStream<BackendEvent> { bus.subscribe() }

    // MARK: Preview hooks

    /// Records a copy, like the real clipboard watcher (the browser mock listened for `copy`).
    public func recordCopy(_ text: String) {
        guard state.settings.clipboardEnabled else { return }
        let now = Date()
        let entry: ClipEntry
        if var existing = clips.first(where: { $0.text == text }) {
            existing.copiedAt = now
            existing.copyCount += 1
            existing.timings = Array((existing.timings + [now]).suffix(64))
            entry = existing
        } else {
            let isColor = text.range(of: "^#[0-9a-f]{3,8}$", options: [.regularExpression, .caseInsensitive]) != nil
            let isLink = text.range(of: "^https?://", options: .regularExpression) != nil && text.rangeOfCharacter(from: .whitespacesAndNewlines) == nil
            entry = ClipEntry(
                kind: isLink ? .link : isColor ? .color : .text,
                text: text,
                textLen: text.utf16.count,
                lineCount: text.split(separator: "\n", omittingEmptySubsequences: false).count,
                color: isColor ? text.lowercased() : nil,
                types: ["public.utf8-plain-text"],
                size: UInt64(text.utf16.count),
                source: ClipSource(bundleId: "sh.saeed.kudzu", name: "Kudzu"),
                hash: UUID().lowercased,
                firstCopiedAt: now,
                copiedAt: now,
                timings: [now]
            )
        }
        clips = [entry] + clips.filter { $0.id != entry.id }
        bus.send(.clipboard(.upsert(entry: entry)))
    }

    /// Shows a permission state (`kudzu:preview-permission` in the browser preview).
    public func previewPermission(_ mode: PermissionPreview) {
        state.status = Self.permissionStatus(mode)
        bus.send(.status(state.status))
    }

    private static func permissionStatus(_ mode: PermissionPreview) -> EngineStatus {
        let granted = mode != .missing && mode != .revoked
        var s = status(paused: mode == .paused)
        s.permissions = PermissionStatus(accessibility: granted, inputMonitoring: false)
        s.tap = granted && mode != .restart ? .active : .off
        s.canPostEvents = granted && mode != .restart
        s.canExpand = mode == .allowed
        s.secureInput = mode == .secure
        s.summary = mode == .paused ? "Paused" : mode == .secure ? "Secure Input on — expansion blocked" : granted ? "Ready" : "Needs Accessibility"
        return s
    }

    private static func status(paused: Bool) -> EngineStatus {
        EngineStatus(
            canPostEvents: true,
            canExpand: !paused,
            paused: paused,
            tap: .active,
            permissions: PermissionStatus(accessibility: true, inputMonitoring: true),
            secureInput: false,
            secureInputOwner: nil,
            summary: paused ? "Paused" : "Ready"
        )
    }

    func commit() { bus.send(.library(state.library)) }
    func commitSettings() { bus.send(.settings(state.settings)) }

    // MARK: State & library

    public func getState() -> AppState { state }

    public func createSnippet(draft: SnippetDraft) -> Snippet {
        let s = LibraryEditing.createSnippet(in: &state.library, draft: draft, now: MockSeed.iso())
        commit()
        return s
    }

    public func saveSnippet(_ snippet: Snippet) throws -> Snippet {
        let s = try LibraryEditing.saveSnippet(snippet, in: &state.library, now: MockSeed.iso())
        commit()
        return s
    }

    public func deleteSnippets(ids: [UUID]) {
        LibraryEditing.deleteSnippets(ids: ids, in: &state.library)
        commit()
    }

    public func restoreSnippets(_ snippets: [Snippet]) {
        LibraryEditing.restoreSnippets(snippets, in: &state.library)
        commit()
    }

    public func duplicateSnippet(id: UUID) throws -> Snippet {
        let copy = try LibraryEditing.duplicateSnippet(id: id, in: &state.library, now: MockSeed.iso())
        commit()
        return copy
    }

    public func moveSnippets(ids: [UUID], groupId: UUID?) throws {
        try LibraryEditing.moveSnippets(ids: ids, groupID: groupId, in: &state.library, now: MockSeed.iso())
        commit()
    }

    public func createGroup(name: String, parentId: UUID?) throws -> Group {
        let g = try LibraryEditing.createGroup(name: name, parentID: parentId, in: &state.library)
        commit()
        return g
    }

    public func saveGroup(_ group: Group) throws -> Group {
        let saved = try LibraryEditing.saveGroup(group, in: &state.library)
        commit()
        return saved
    }

    public func deleteGroup(id: UUID, deleteSnippets: Bool) {
        LibraryEditing.deleteGroup(id: id, deleteSnippets: deleteSnippets, in: &state.library)
        commit()
    }

    public func moveGroup(id: UUID, parentId: UUID?, index: Int) throws {
        try LibraryEditing.moveGroup(id: id, parentID: parentId, index: index, in: &state.library)
        commit()
    }

    public func findConflicts() -> [TriggerConflict] {
        KudzuCore.findConflicts(snippets: state.library.snippets, groups: state.library.groups)
    }

    // MARK: Settings & engine

    public func updateSettings(_ patch: SettingsPatch) throws -> Settings {
        if patch.onboardingCompleted == true, options.finishRetry, !finishPreviewFailed {
            finishPreviewFailed = true
            throw BackendError("Preview: settings could not be saved.")
        }
        state.settings = patch.applied(to: state.settings)
        commitSettings()
        if let sync = patch.icloudSync {
            bus.send(.sync(sync ? .upToDate(lastSync: KudzuDate.formatMilliseconds(Date())) : .off))
        }
        return state.settings
    }

    public func setPaletteHotkey(accelerator: String) throws -> Settings {
        try updateSettings(SettingsPatch { $0.paletteHotkey = accelerator })
    }

    public func setPaused(_ paused: Bool) -> EngineStatus {
        state.settings.paused = paused
        state.status = Self.status(paused: paused)
        commitSettings()
        bus.send(.status(state.status))
        return state.status
    }

    public func permissionContext() -> PermissionContext {
        PermissionContext(kind: options.permissionKind, appName: "Kudzu", accessibilityName: "Accessibility")
    }

    public func installPermissionIcon(bounds: ScreenRect) -> Bool { false }
    public func beginPermission(kind: PermissionKind) {}
    public func stopPermissionGuide() {}
    public func restartPermissionApp() { previewPermission(.allowed) }
    public func showPermissions(kind: PermissionKind) {}
    public func closePermissions() {}
    public func revealPermissionApp() {}
    public func requestPermission(kind: PermissionKind) {}
    public func openSystemSettings(pane: SystemSettingsPane) {}
    public func engineStatus() -> EngineStatus { state.status }
}
