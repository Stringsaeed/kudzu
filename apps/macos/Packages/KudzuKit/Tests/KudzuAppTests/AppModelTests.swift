import Foundation
import KudzuCore
import Testing
@testable import KudzuApp

/// The optimistic flows of `lib/store.ts` / `lib/clipboard.ts`, on `MockBackend`.
@MainActor @Suite struct AppModelTests {
    private func started(_ options: MockBackend.Options = .init()) async -> (AppModel, MockBackend) {
        let backend = MockBackend(options: options)
        let model = AppModel(backend: backend)
        await model.start()
        return (model, backend)
    }

    /// Waits (up to 2 s) for an event-driven change.
    private func eventually(_ condition: @MainActor () -> Bool) async -> Bool {
        for _ in 0..<200 {
            if condition() { return true }
            try? await Task.sleep(for: .milliseconds(10))
        }
        return condition()
    }

    /// Waits until every event the backend has sent so far has been applied: events can land
    /// after an optimistic update (as in the TypeScript store), so state is eventually consistent.
    private func settle(_ model: AppModel, _ backend: MockBackend) async {
        for _ in 0..<200 {
            let state = await backend.getState()
            let clips = await backend.clipboardList().entries
            if model.library == state.library, model.settings == state.settings, model.status == state.status,
               model.clipboard.entries == clips { return }
            try? await Task.sleep(for: .milliseconds(10))
        }
        Issue.record("model never caught up with the backend")
    }

    // MARK: Loading

    @Test func startLoadsStateAndSelectsTheFirstSnippet() async {
        let (model, _) = await started()
        #expect(model.ready)
        #expect(model.platform == .web)
        #expect(model.library.snippets.count == 15)
        #expect(model.library.groups.map(\.name) == ["Work", "Email", "Developer", "Personal"])
        #expect(model.selectedId == model.library.snippets.first?.id)
        #expect(model.usage.count == 12)
        #expect(model.clipboard.ready && model.clipboard.entries.count == 85)
        #expect(model.clipboard.entries == model.clipboard.entries.sorted { $0.copiedAt > $1.copiedAt })
        #expect(model.updateState == .idle)
        if case .upToDate = model.syncStatus {} else { Issue.record("sync status \(model.syncStatus)") }
    }

    @Test func initialViewDeepLinks() async {
        let model = AppModel(backend: MockBackend(), initialView: .settings(section: .about))
        await model.start()
        #expect(model.view == .settings(section: .about))
    }

    // MARK: Snippets

    @Test func newSnippetSelectsAndFocusesName() async throws {
        let (model, _) = await started()
        model.filter = "sig"
        let s = try await model.newSnippet()
        #expect(model.library.snippets.first?.id == s.id)
        #expect(model.selectedId == s.id)
        #expect(model.focusNameOf == s.id)
        #expect(model.filter.isEmpty)
        model.consumeFocusName()
        #expect(model.focusNameOf == nil)
        // The backend's `library` event doesn't duplicate it.
        #expect(await eventually { model.library.snippets.filter { $0.id == s.id }.count == 1 && model.library.snippets.count == 16 })
    }

    @Test func newSnippetFollowsTheView() async throws {
        let (model, _) = await started()
        let work = try #require(model.library.groups.first)
        model.setView(.group(id: work.id))
        #expect(try await model.newSnippet().groupID == work.id)
        #expect(model.view == .group(id: work.id))

        model.setView(.favorites)
        #expect(try await model.newSnippet().isFavorite)
        #expect(model.view == .favorites)

        model.setView(.insights)
        let plain = try await model.newSnippet(SnippetDraft(name: "Named"))
        #expect(plain.name == "Named" && !plain.isFavorite && plain.groupID == nil)
        #expect(model.view == .all)
    }

    @Test func deletePicksTheNeighbourAndRestoreReselects() async throws {
        let (model, backend) = await started()
        let before = model.library.snippets
        model.select(before[1].id)
        let removed = try await model.deleteSnippets([before[1].id])
        #expect(removed == [before[1]])
        #expect(model.selectedId == before[2].id)
        #expect(!model.library.snippets.contains { $0.id == before[1].id })
        await settle(model, backend)

        // Deleting the last one selects the new last.
        let last = try #require(model.library.snippets.last)
        model.select(last.id)
        _ = try await model.deleteSnippets([last.id])
        #expect(model.selectedId == model.library.snippets.last?.id)

        try await model.restoreSnippets(removed)
        #expect(model.selectedId == before[1].id)
        #expect(await eventually { model.library.snippets.contains { $0.id == before[1].id } })
    }

    @Test func deletingAnUnselectedSnippetKeepsTheSelection() async throws {
        let (model, _) = await started()
        let selected = model.selectedId
        _ = try await model.deleteSnippets([model.library.snippets[3].id])
        #expect(model.selectedId == selected)
    }

    @Test func togglesConvergeToCommittedStateWithoutReplayingOlderResults() async throws {
        let (model, backend) = await started()
        let s = model.library.snippets[1]
        try await model.toggleFavorite(s.id)
        await settle(model, backend)
        #expect(model.snippet(s.id)?.isFavorite == !s.isFavorite)
        try await model.toggleEnabled(s.id)
        await settle(model, backend)
        #expect(model.snippet(s.id)?.isEnabled == !s.isEnabled)
        #expect(model.snippet(s.id)?.isFavorite == !s.isFavorite)
    }

    @Test func duplicateSelectsTheCopy() async throws {
        let (model, backend) = await started()
        let s = model.library.snippets[0]
        try await model.duplicate(s.id)
        let copy = try #require(model.selectedSnippet)
        #expect(copy.name == "\(s.name) copy" && copy.triggers == s.triggers.map { $0 + "2" })
        #expect(model.selectedId == copy.id && model.focusNameOf == copy.id)
        await settle(model, backend)
        #expect(model.library.snippets[0].id == s.id)
        #expect(model.library.snippets[1].id == copy.id)
    }

    @Test func groupsCreateMoveAndDelete() async throws {
        let (model, backend) = await started()
        let g = try await model.createGroup(name: "New", parentId: nil)
        #expect(model.view == .group(id: g.id))
        await settle(model, backend)
        let ids = Array(model.library.snippets.prefix(2).map(\.id))
        try await model.moveSnippets(ids, to: g.id)
        #expect(model.library.snippets.prefix(2).allSatisfy { $0.groupID == g.id })
        try await model.deleteGroup(g.id, deleteSnippets: false)
        #expect(model.view == .all)
        #expect(await eventually { !model.library.groups.contains { $0.id == g.id } && model.snippet(ids[0])?.groupID == nil })
    }

    @Test func moveGroupReindexesSiblingsAndDoesNotOptimisticallyMutateOnFailure() async throws {
        let (model, backend) = await started()
        let work = try #require(model.library.groups.first { $0.name == "Work" })
        let email = try #require(model.library.groups.first { $0.name == "Email" })

        #expect(try await !model.moveGroup(work.id, into: email.id))
        #expect(try await model.moveGroup(email.id, into: nil))
        await settle(model, backend)

        let movedEmail = try #require(model.library.groups.first { $0.id == email.id })
        #expect(movedEmail.parentID == nil && movedEmail.sortIndex == 3)

        #expect(try await model.moveGroup(email.id, into: nil, index: 1))
        await settle(model, backend)
        let roots = model.library.groups
            .filter { $0.parentID == nil }
            .sorted { $0.sortIndex < $1.sortIndex }
        #expect(roots.map(\.name) == ["Work", "Email", "Developer", "Personal"])
        #expect(roots.map(\.sortIndex) == [0, 1, 2, 3])

        let beforeFailure = model.library.groups
        await #expect(throws: LibraryEditError.parentGroupNotFound) {
            try await model.moveGroup(email.id, into: UUID())
        }
        #expect(model.library.groups == beforeFailure)
    }

    // MARK: Settings & events

    @Test func settingsAndPauseUpdateFromTheBackend() async throws {
        let (model, backend) = await started()
        try await model.updateSettings(SettingsPatch { $0.theme = .light })
        #expect(model.settings.theme == .light)
        await settle(model, backend)
        try await model.setPaused(true)
        #expect(model.settings.paused && model.status.paused && model.status.summary == "Paused")

        try await model.updateSettings(SettingsPatch { $0.icloudSync = false })
        #expect(await eventually { model.syncStatus == .off })
    }

    @Test func rejectedSettingsKeepCommittedStateAndCanBeRetried() async {
        var options = MockBackend.Options()
        options.finishRetry = true
        options.onboarding = true
        let (model, _) = await started(options)
        #expect(!model.settings.onboardingCompleted)
        await #expect(throws: BackendError("Preview: settings could not be saved.")) {
            try await model.updateSettings(SettingsPatch { $0.onboardingCompleted = true })
        }
        #expect(!model.settings.onboardingCompleted)
        try? await model.updateSettings(SettingsPatch { $0.onboardingCompleted = true })
        #expect(model.settings.onboardingCompleted)
    }

    @Test func eventsReplaceState() async throws {
        let (model, backend) = await started()
        var s = model.library.snippets[2]
        s.name = "Renamed elsewhere"
        _ = try await backend.saveSnippet(s)
        #expect(await eventually { model.snippet(s.id)?.name == "Renamed elsewhere" })

        _ = await backend.setPaused(true)
        #expect(await eventually { model.status.paused })

        await backend.recordCopy("hello from a test")
        #expect(await eventually { model.clipboard.entries.first?.text == "hello from a test" })
    }

    @Test func navigateSelectsTheSnippetInItsGroup() async throws {
        let (model, _) = await started()
        let grouped = try #require(model.library.snippets.first { $0.groupID != nil })
        model.filter = "x"
        model.handle(.navigate(.init(snippetId: grouped.id)))
        #expect(model.selectedId == grouped.id)
        #expect(model.view == .group(id: try #require(grouped.groupID)))
        #expect(model.filter.isEmpty)
        model.handle(.navigate(.init(view: .settings(section: .about))))
        #expect(model.view == .settings(section: .about))
    }

    @Test func newSnippetEventPrefixesTheTrigger() async throws {
        let (model, _) = await started()
        model.handle(.newSnippet(.init(trigger: "hi")))
        #expect(await eventually { model.library.snippets.first?.triggers == [";hi"] })
        model.handle(.newSnippet(.init(name: "From palette", trigger: ";yo")))
        #expect(await eventually { model.library.snippets.first?.triggers == [";yo"] })
        #expect(model.focusNameOf == model.library.snippets.first?.id)
    }

    // MARK: Clipboard

    @Test func clipboardRemoveRestorePinAndClear() async throws {
        let (model, backend) = await started()
        let first = try #require(model.clipboard.entries.first)
        let gone = try await model.removeClips([first.id])
        #expect(gone.map(\.id) == [first.id])
        #expect(!model.clipboard.entries.contains { $0.id == first.id })
        await settle(model, backend)
        try await model.restoreClips(gone)
        #expect(model.clipboard.entries.first?.id == first.id)
        await settle(model, backend)

        try await model.setClipPinned(first.id, pinned: true)
        #expect(model.clipboard.entries.first?.pinned == true)
        await settle(model, backend)
        try await model.setClipSecret(first.id, secret: true)
        #expect(model.clipboard.entries.first?.isSecret == true)
        await settle(model, backend)

        let pinned = model.clipboard.entries.filter(\.pinned).count
        let cleared = try await model.clearClipboard(includePinned: false)
        #expect(cleared == 85 - pinned)
        #expect(model.clipboard.entries.count == pinned && model.clipboard.entries.allSatisfy(\.pinned))
    }

    @Test func clipEventsKeepNewestFirst() async {
        let (model, _) = await started(.init())
        var old = model.clipboard.entries[5]
        old.copiedAt = Date().addingTimeInterval(60)
        model.applyClipEvent(.upsert(entry: old))
        #expect(model.clipboard.entries.first?.id == old.id)
        model.applyClipEvent(.remove(ids: [old.id]))
        #expect(!model.clipboard.entries.contains { $0.id == old.id })
        model.applyClipEvent(.reset(entries: []))
        #expect(model.clipboard.entries.isEmpty)
    }

    // MARK: Mock behaviour

    @Test func mockUpdateAndMedia() async throws {
        var options = MockBackend.Options()
        options.update = true
        options.checkDelay = .zero
        let backend = MockBackend(options: options)
        if case let .ready(version, notes, _) = try await backend.checkForUpdates() {
            #expect(version == "0.2.0" && notes?.hasPrefix("## [0.2.0]") == true)
        } else { Issue.record("expected a ready update") }
        let image = try #require(await backend.clipboardList().entries.first { $0.kind == .image })
        let png = try #require(await backend.clipboardMedia(kind: .thumb, id: image.id))
        #expect(png.starts(with: [0x89, 0x50, 0x4E, 0x47]))
    }

    @Test func mockRendersPreviews() async throws {
        let backend = MockBackend()
        let r = try await backend.renderPreview(content: "Hi {{field name=\"Name\"}}{{cursor}} {{clipboard}}", snippetId: nil, formValues: ["Name": "Ada"])
        #expect(r.text == "Hi Ada copied text")
        #expect(r.cursorOffset == 6)
    }
}
