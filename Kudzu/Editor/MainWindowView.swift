import KudzuCore
import KudzuUI
import SwiftUI

/// The library stays mounted while settings is presented inside the same window.
struct MainWindowView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        MainWindowContent(model: model)
    }
}

private struct MainWindowContent: View {
    let model: AppModel
    @State private var library: LibraryModel
    @State private var browser: BrowserState

    @SceneStorage("main.scope") private var scopeRaw = SidebarScope.all.rawValue
    @SceneStorage("main.selection") private var selectionRaw = ""
    @SceneStorage("main.settingsSection") private var settingsSectionRaw = SettingsSection.general.rawValue
    @State private var started = false
    @FocusState private var searchFocused: Bool

    init(model: AppModel) {
        self.model = model
        _library = State(initialValue: model.library)
        _browser = State(initialValue: model.browser)
    }

    private var lastSettingsSection: SettingsSection {
        SettingsSection(rawValue: settingsSectionRaw) ?? .general
    }

    private var windowMode: Binding<MainWindowMode> {
        Binding(get: { model.mainWindowMode }, set: { model.mainWindowMode = $0 })
    }

    var body: some View {
        MainWindowShell(
            shellMode: .window,
            windowMode: windowMode,
            lastSettingsSection: lastSettingsSection,
            storeError: model.storeError,
            searchFocus: $searchFocused
        ) { section in
            SettingsPage(section: section)
        }
        .background {
            MainWindowKeyMonitor(keyContext: keyContext(for:), perform: performKeyAction)
            Button("Find snippets") { focusSearch(selectAll: true) }
                .keyboardShortcut("f", modifiers: .command)
                .buttonStyle(.plain)
                .frame(width: 0, height: 0)
                .opacity(0)
                .focusable(false)
                .accessibilityHidden(true)
                .disabled(model.mainWindowMode != .library)
        }
        .background(DockPolicyAccessor())
        .background(LibraryUndoAccessor(library: library))
        .environment(library)
        .environment(browser)
        .focusedSceneValue(\.mainWindowActions, actions)
        .confirmationDialog(
            deleteTitle,
            isPresented: Binding(get: { !browser.deleteRequest.isEmpty }, set: { if !$0 { browser.deleteRequest = [] } }),
            titleVisibility: .visible
        ) {
            Button("Delete", role: .destructive) {
                library.deleteSnippets(browser.deleteRequest)
                browser.prune(using: library)
                browser.deleteRequest = []
            }
            Button("Cancel", role: .cancel) { browser.deleteRequest = [] }
        } message: {
            Text("This also removes their usage history. Undo can restore deleted items while this window is open; conflicting changes will pause undo for review.")
        }
        .task { await start() }
        .onDisappear {
            // The window opens on the library next time, unless ⌘, opens it.
            model.mainWindowMode = .library
        }
        .onChange(of: browser.scope) { _, scope in
            scopeRaw = scope.rawValue
            // Stats hides the editor without dropping the selection, so coming back restores it.
            if scope != .stats { reconcileSelection() }
        }
        .onChange(of: browser.query) { _, _ in
            if browser.scope != .stats { reconcileSelection() }
        }
        .onChange(of: browser.selection) { _, selection in
            selectionRaw = selection.count == 1 ? (selection.first?.uuidString ?? "") : ""
        }
        .onChange(of: model.mainWindowMode) { _, mode in
            if case .settings(let section) = mode { settingsSectionRaw = section.rawValue }
        }
        .onChange(of: library.revision) { browser.prune(using: library) }
        .onChange(of: library.recoveryDrafts.map(\.id)) { browser.prune(using: library) }
        .onChange(of: model.pendingNewSnippetContent) { if started { consumePendingContent() } }
        .onChange(of: model.pendingSelectSnippetID) { if started { consumePendingSelection() } }
        .onChange(of: model.mainWindowRequest) { if started { consumeWindowRequest() } }
        .onChange(of: browser.editorCanDiscard) { _, dirty in
            if started && !dirty { consumePendingContent() }
        }
    }

    // MARK: - Commands

    private var actions: MainWindowActions {
        let isSettings = model.mainWindowMode != .library
        let canEditSelection = !isSettings && browser.scope != .stats && !browser.selection.isEmpty
        let queryIsBlank = browser.query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        return MainWindowActions(
            newSnippet: {
                // The settings modal stays up. ⌘N creates only from the library, or after a closed window opens.
                guard !isSettings else { return }
                browser.newSnippet(library: library)
            },
            openSettings: {
                if !isSettings { model.showSettings(lastSettingsSection) }
            },
            duplicate: canEditSelection ? {
                let copies = library.duplicate(Array(browser.selection))
                if !copies.isEmpty { browser.replaceSelection(Set(copies)) }
            } : nil,
            leaveSettings: isSettings ? {
                guard let window = NSApp.mainWindow,
                      SettingsModalFocusSession.canHandleKeys(in: window, keyWindow: NSApp.keyWindow,
                          hasNestedPresentation: SettingsModalFocusSession.hasNestedPresentation(in: window)) else { return }
                model.mainWindowMode = .library
            } : nil,
            focusSearch: { focusSearch(selectAll: true) },
            focusSidebar: { browser.paneRequest = .sidebar },
            focusResults: { focusResults() },
            focusEditor: { focusEditor(.name) },
            clearSearch: queryIsBlank ? nil : { clearSearch() },
            discardChanges: !isSettings && browser.editorCanDiscard ? {
                browser.editorBridge?.discard()
            } : nil,
            saveChanges: !isSettings && browser.editorCanDiscard ? { browser.editorBridge?.save() } : nil,
            copyExpanded: canEditSelection && browser.selection.count == 1 ? { browser.editorBridge?.copyExpanded() } : nil,
            copySource: canEditSelection && browser.selection.count == 1 ? { browser.editorBridge?.copySource() } : nil,
            toggleFavorite: canEditSelection ? { toggleFavorite() } : nil,
            deleteSelection: canEditSelection ? { deleteSelection() } : nil
        )
    }

    /// Drops selected snippets the current scope and query no longer show.
    ///
    /// Uses a deferred selection as the basis, so a refused navigation is not thrown away when the
    /// filter changes in the same turn. Does not invent a selection.
    private func reconcileSelection() {
        guard browser.scope != .stats else { return }
        let visible = library.snippets(in: browser.scope, query: browser.query, sort: .name).map(\.id)
        let basis = browser.deferredSelection ?? browser.selection
        let next = KzSelectionReconciler.reconciled(basis, visible: visible)
        guard next != basis else { return }
        browser.replaceSelection(next)
    }

    /// The search field filters the snippet list. Settings keeps the modal; Stats returns to All first.
    private func focusSearch(selectAll: Bool) {
        guard model.mainWindowMode == .library else { return }
        if browser.scope == .stats { browser.scope = .all }
        if selectAll { browser.searchSelectAllToken += 1 }
        browser.paneRequest = .search
    }

    private func focusResults() {
        guard model.mainWindowMode == .library else { return }
        showLibrary()
        browser.paneRequest = .results
    }

    private func focusEditor(_ target: EditorFocusTarget) {
        guard model.mainWindowMode == .library else { return }
        showLibrary()
        browser.paneRequest = .editor(target)
    }

    private func clearSearch() {
        guard model.mainWindowMode == .library else { return }
        showLibrary()
        browser.query = ""
        reconcileSelection()
        browser.paneRequest = .results
    }

    private func showLibrary() {
        if model.mainWindowMode != .library { model.mainWindowMode = .library }
        if browser.scope == .stats { browser.scope = .all }
    }

    private func toggleFavorite() {
        if let bridge = browser.editorBridge, browser.selection == [bridge.owner] {
            bridge.toggleFavorite()
            return
        }
        let ids = Array(browser.selection)
        let snippets = ids.compactMap { library.snippetsByID[$0] }
        let allFavorite = !snippets.isEmpty && snippets.allSatisfy(\.isFavorite)
        library.update(ids) { $0.isFavorite = !allFavorite }
    }

    private func deleteSelection() {
        if let bridge = browser.editorBridge, browser.selection == [bridge.owner] {
            bridge.delete()
            return
        }
        browser.requestDelete(Array(browser.selection), in: library)
    }

    private func keyContext(for window: NSWindow) -> KzLibraryKeyContext {
        let control = MainWindowFocus.control(in: window)
        let stored = browser.selection.first.flatMap { browser.uncommittedTriggers[$0]?.text }
        let composing = (window.firstResponder as? NSTextView)?.hasMarkedText() == true
        let panelChild = (window.childWindows ?? []).contains { $0.isVisible }
        let transient = composing
            || KzMenu.isOpen
            || window.attachedSheet != nil
            || NSApp.modalWindow != nil
            || panelChild
            || !browser.deleteRequest.isEmpty
        return KzLibraryKeyContext(
            control: control,
            inSettings: model.mainWindowMode != .library,
            queryIsEmpty: browser.query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
            hasSingleSelection: browser.selection.count == 1,
            triggerDraftPending: MainWindowFocus.triggerDraftPending(control: control, window: window, stored: stored),
            transientOpen: transient
        )
    }

    private func performKeyAction(_ action: KzLibraryAction) {
        switch action {
        case .passthrough:
            break
        case .focus(let pane):
            switch pane {
            case .sidebar: browser.paneRequest = .sidebar
            case .search: focusSearch(selectAll: false)
            case .results: focusResults()
            case .editorName: focusEditor(.name)
            case .editorTriggers: focusEditor(.trigger)
            case .editorContent: focusEditor(.content)
            }
        case .clearSearch:
            clearSearch()
        case .cancelTriggerDraft:
            cancelTriggerDraft()
        case .deleteSelection:
            deleteSelection()
        }
    }

    /// Drops uncommitted trigger text only. The field editor is cleared now; the binding update follows.
    private func cancelTriggerDraft() {
        guard let id = browser.selection.first else { return }
        browser.clearTriggerEntry(id)
        if let editor = NSApp.keyWindow?.firstResponder as? NSTextView {
            editor.string = ""
        }
    }

    private var deleteTitle: String {
        let ids = browser.deleteRequest
        if ids.count == 1, let snippet = library.snippetsByID[ids[0]] {
            return "Delete “\(snippet.name)”?"
        }
        return "Delete \(ids.count) snippets?"
    }

    // MARK: - Startup

    private func start() async {
        library.ensureLoaded()
        #if DEBUG
        DebugSeed.seedIfRequested(library: library)
        #endif
        browser.scope = SidebarScope(rawValue: scopeRaw) ?? .all
        if case .group(let id) = browser.scope, library.tree.group(id) == nil { browser.scope = .all }
        if let id = UUID(uuidString: selectionRaw), library.snippetsByID[id] != nil {
            browser.selection = [id]
        }
        started = true
        consumePendingContent()
        consumePendingSelection()
        consumeWindowRequest()
        await library.observeChanges()
    }

    /// Menu commands issued while this window was closed or not key.
    ///
    /// The request is cleared before it runs, so opening the window cannot create a second snippet.
    private func consumeWindowRequest() {
        guard let request = model.mainWindowRequest else { return }
        model.mainWindowRequest = nil
        switch request {
        case .newSnippet:
            // The window was closed or not showing settings. Opening it is allowed to create.
            model.mainWindowMode = .library
            browser.newSnippet(library: library)
        case .focusSearch:
            model.mainWindowMode = .library
            focusSearch(selectAll: true)
        case .focusSidebar:
            browser.paneRequest = .sidebar
        case .focusResults:
            focusResults()
        case .focusEditor:
            focusEditor(.name)
        case .clearSearch:
            clearSearch()
        }
    }

    /// "New Snippet from Selection" hands its text over through `AppModel`.
    private func consumePendingContent() {
        guard let content = model.pendingNewSnippetContent else { return }
        model.mainWindowMode = .library
        if browser.newSnippet(
            library: library,
            content: content,
            nameHint: UniqueName.name(fromContent: content, fallback: "New Snippet"),
            focus: .trigger
        ) { model.pendingNewSnippetContent = nil }
    }

    /// The palette's ⌘E hands over a snippet to select.
    private func consumePendingSelection() {
        guard let id = model.pendingSelectSnippetID else { return }
        model.pendingSelectSnippetID = nil
        model.mainWindowMode = .library
        // The palette may know about a snippet this window hasn't reloaded yet.
        if library.snippetsByID[id] == nil { library.reload() }
        browser.reveal(id, in: library)
    }
}

// MARK: - Shell

/// The persistent library panes and optional settings overlay, using the library and browser environment.
///
/// Separate from `MainWindowContent` so snapshots can render it embedded, without an `AppModel`.
struct MainWindowShell<SettingsDetail: View>: View {
    @Environment(LibraryModel.self) private var library
    @Environment(BrowserState.self) private var browser
    @Environment(\.kzSnapshotRendering) private var snapshotRendering

    let shellMode: KzAppShellMode
    @Binding var windowMode: MainWindowMode
    let lastSettingsSection: SettingsSection
    let storeError: String?
    let searchFocus: FocusState<Bool>.Binding
    @ViewBuilder let settingsDetail: (SettingsSection) -> SettingsDetail

    private var settingsSection: SettingsSection? {
        if case .settings(let section) = windowMode { section } else { nil }
    }

    var body: some View {
        KzAppShell(mode: shellMode) {
            SidebarView(isSettings: settingsSection != nil) { scope in
                browser.chooseScope(scope)
                windowMode = .library
            }
        } sidebarHeader: {
            #if DEBUG
            DebugDataButton()
            #endif
        } sidebarFooter: {
            KzSidebarItem("Settings", icon: .sliders, shortcut: "⌘,", isActive: settingsSection != nil) {
                if settingsSection == nil { windowMode = .settings(lastSettingsSection) }
            }
        } list: {
            if browser.scope == .stats {
                StatsLeaderboardView()
            } else {
                SnippetListView(searchFocus: searchFocus)
            }
        } detail: {
            detail
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
                .safeAreaInset(edge: .bottom, spacing: 0) { banners }
        }
        .allowsHitTesting(settingsSection == nil)
        .accessibilityHidden(settingsSection != nil)
        .overlay {
            if let section = settingsSection {
                SettingsModalView(
                    selection: Binding(get: { section }, set: { windowMode = .settings($0) }),
                    close: { windowMode = .library },
                    content: settingsDetail
                )
            }
        }
    }

    @ViewBuilder
    private var detail: some View {
        if browser.scope == .stats {
            StatsView()
        } else if browser.selection.count == 1, let id = browser.selection.first, let snippet = library.editableSnippet(id) {
            SnippetEditorView(snippet: snippet, library: library)
                .id(id)
        } else if browser.selection.count > 1 {
            CenteredEmptyState {
                KzEmptyState("\(browser.selection.count) snippets selected", icon: .stack) {
                    KzMessage("Right-click to move, favorite or delete them together, or press \(kbd: "⌫").")
                }
            }
        } else {
            CenteredEmptyState {
                KzEmptyState("No snippet selected") {
                    KzMessage("Choose a snippet to edit it, or press \(kbd: "⌘N") to create one.")
                }
            }
        }
    }

    // MARK: Banners

    @ViewBuilder
    private var banners: some View {
        ForEach(browser.uncommittedTriggers.keys.filter { library.editableSnippet($0) == nil }.sorted { $0.uuidString < $1.uuidString }, id: \.self) { id in
            HStack {
                Text("An unfinished trigger belongs to a deleted snippet.").kzTextStyle(.caption)
                KzButton("Discard trigger", variant: .ghost, size: .sm) { browser.clearTriggerEntry(id) }
            }
            .padding(KzSpace.s2)
        }
        ForEach(library.deletionRetries) { retry in
            VStack(alignment: .leading, spacing: KzSpace.s2) {
                Text(retry.message)
                Button(retry.title) { library.retryLibraryDeletion(retry.id) }
            }
            .padding(KzSpace.s3)
        }
        ForEach(library.recoveryDrafts) { draft in
            Button("Review unsaved changes: \(draft.name)") {
                windowMode = .library
                browser.scope = .all
                browser.selection = [draft.id]
            }
            .padding(KzSpace.s2)
        }
        if storeError != nil || library.errorMessage != nil {
            VStack(spacing: KzSpace.s2) {
                if let storeError {
                    WindowBanner(message: storeError + " Changes will not be saved.", tone: .danger, dismiss: nil)
                }
                if let message = library.errorMessage {
                    WindowBanner(message: message, tone: .warning) { library.errorMessage = nil }
                }
            }
            .padding(KzSpace.s3)
        }
    }
}

/// Centres an empty state in its pane, with the header row's height kept clear for dragging the window.
struct CenteredEmptyState<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        VStack(spacing: 0) {
            Color.clear.frame(height: KzSize.titlebar).kzWindowDragArea()
            Spacer(minLength: 0)
            content.frame(maxWidth: .infinity)
            Spacer(minLength: 0)
            Color.clear.frame(height: KzSize.titlebar)
        }
    }
}

// MARK: - Banner

/// A notice at the bottom of the detail pane: an error the user should know about, with an optional Dismiss.
struct WindowBanner: View {
    enum Tone { case warning, danger }

    var message: String
    var tone: Tone
    var dismiss: (() -> Void)?

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.md)
        HStack(alignment: .center, spacing: KzSpace.s2) {
            KzIcon(tone == .danger ? .error : .warning, size: 14)
                .foregroundStyle(tone == .danger ? KzColor.danger : KzColor.warning)
            Text(message)
                .kzTextStyle(.callout)
                .foregroundStyle(KzColor.ink)
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
            if let dismiss {
                KzButton("Dismiss", variant: .ghost, size: .sm, action: dismiss)
            }
        }
        .padding(.horizontal, KzSpace.s3)
        .padding(.vertical, KzSpace.s2)
        .background(tone == .danger ? KzColor.dangerSoft : KzColor.warningSoft, in: shape)
        .overlay(shape.strokeBorder(KzColor.line))
        .accessibilityElement(children: .combine)
    }
}

// MARK: - Debug

#if DEBUG
/// DEBUG builds only: loads sample data from the sidebar header.
private struct DebugDataButton: View {
    @Environment(LibraryModel.self) private var library

    var body: some View {
        KzIconButton(.bug, label: "Sample data (debug builds only)", size: .sm, menuTitle: "Sample data",
                     placement: .pullDown(.trailing)) {
            KzMenuItem("Seed demo data", icon: .plus) { DebugSeed.demo(library: library) }
            KzMenuItem("Seed 3,000 snippets", icon: .stack) { DebugSeed.bulk(3000, library: library) }
            KzMenuSeparator()
            KzMenuItem("Remove demo data", icon: .trash, role: .danger) { DebugSeed.removeDemoData(library: library) }
        }
    }
}
#endif
