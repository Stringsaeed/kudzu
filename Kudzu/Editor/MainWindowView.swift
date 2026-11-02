import KudzuCore
import KudzuUI
import SwiftUI

/// The one Kudzu window: sidebar, list pane and detail pane, in library or settings mode.
struct MainWindowView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        MainWindowContent(model: model)
    }
}

private struct MainWindowContent: View {
    let model: AppModel
    @State private var library: LibraryModel
    @State private var browser = BrowserState()

    @SceneStorage("main.scope") private var scopeRaw = SidebarScope.all.rawValue
    @SceneStorage("main.selection") private var selectionRaw = ""
    @SceneStorage("main.settingsSection") private var settingsSectionRaw = SettingsSection.general.rawValue
    @State private var started = false
    @FocusState private var searchFocused: Bool

    init(model: AppModel) {
        self.model = model
        _library = State(initialValue: LibraryModel(store: model.store))
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
        .background { FindShortcut(action: focusSearch) }
        .background(DockPolicyAccessor())
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
            Text("This also removes their usage history. You can't undo this.")
        }
        .task { await start() }
        .onDisappear {
            // The window opens on the library next time, unless ⌘, opens it.
            model.mainWindowMode = .library
        }
        .onChange(of: browser.scope) { _, scope in
            scopeRaw = scope.rawValue
            // Keep the editor in step with the list: drop selected snippets the new scope doesn't show.
            let visible = Set(library.snippets(in: scope, query: "", sort: .name).map(\.id))
            if scope != .stats { browser.selection.formIntersection(visible) }
        }
        .onChange(of: browser.selection) { _, selection in
            selectionRaw = selection.count == 1 ? (selection.first?.uuidString ?? "") : ""
        }
        .onChange(of: model.mainWindowMode) { _, mode in
            if case .settings(let section) = mode { settingsSectionRaw = section.rawValue }
        }
        .onChange(of: library.revision) { browser.prune(using: library) }
        .onChange(of: model.pendingNewSnippetContent) { if started { consumePendingContent() } }
        .onChange(of: model.pendingSelectSnippetID) { if started { consumePendingSelection() } }
    }

    // MARK: - Commands

    private var actions: MainWindowActions {
        let isSettings = model.mainWindowMode != .library
        let canDuplicate = !isSettings && browser.scope != .stats && !browser.selection.isEmpty
        return MainWindowActions(
            newSnippet: {
                model.mainWindowMode = .library
                browser.newSnippet(library: library)
            },
            openSettings: {
                if !isSettings { model.showSettings(lastSettingsSection) }
            },
            duplicate: canDuplicate ? {
                let copies = library.duplicate(Array(browser.selection))
                if !copies.isEmpty { browser.selection = Set(copies) }
            } : nil,
            leaveSettings: isSettings ? { model.mainWindowMode = .library } : nil
        )
    }

    /// ⌘F: the search field filters the snippet list, so leave settings or Stats first.
    private func focusSearch() {
        model.mainWindowMode = .library
        if browser.scope == .stats { browser.scope = .all }
        Task {
            // The field may only just have appeared.
            try? await Task.sleep(for: .milliseconds(30))
            searchFocused = true
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
        await library.observeChanges()
    }

    /// "New Snippet from Selection" hands its text over through `AppModel`.
    private func consumePendingContent() {
        guard let content = model.pendingNewSnippetContent else { return }
        model.pendingNewSnippetContent = nil
        model.mainWindowMode = .library
        browser.newSnippet(
            library: library,
            content: content,
            nameHint: UniqueName.name(fromContent: content, fallback: "New Snippet"),
            focus: .trigger
        )
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

/// ⌘F focuses the list's search field. A view-level shortcut, so it wins over the Edit menu's Find,
/// as the old `.searchable` field did.
private struct FindShortcut: View {
    let action: () -> Void

    var body: some View {
        Button("Find snippets", action: action)
            .keyboardShortcut("f", modifiers: .command)
            .buttonStyle(.plain)
            .frame(width: 0, height: 0)
            .opacity(0)
            .focusable(false)
            .accessibilityHidden(true)
    }
}

// MARK: - Shell

/// The window's three panes for a mode, built from `LibraryModel` and `BrowserState` in the environment.
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

    private var shellChrome: KzAppShellChrome {
        settingsSection == nil ? .library : .settings
    }

    var body: some View {
        KzAppShell(mode: shellMode, chrome: shellChrome) {
            SidebarView(isSettings: settingsSection != nil) { scope in
                browser.scope = scope
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
            if let section = settingsSection {
                SettingsSectionList(
                    selection: Binding(get: { section }, set: { windowMode = .settings($0) }),
                    returnToLibrary: { windowMode = .library }
                )
            } else if browser.scope == .stats {
                StatsLeaderboardView()
            } else {
                SnippetListView(searchFocus: searchFocus)
            }
        } detail: {
            detail
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
                .safeAreaInset(edge: .bottom, spacing: 0) { banners }
        }
    }

    @ViewBuilder
    private var detail: some View {
        if let section = settingsSection {
            // Settings pages own their scrolling and padding (see `SettingsPageLayout`).
            settingsDetail(section)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .background(KzColor.surface)
            .id(section)
        } else if browser.scope == .stats {
            StatsView()
        } else if browser.selection.count == 1, let id = browser.selection.first, let snippet = library.snippetsByID[id] {
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

// MARK: - Settings section list

/// The list pane in settings mode: one row per section.
private struct SettingsSectionList: View {
    @Binding var selection: SettingsSection
    var returnToLibrary: () -> Void

    private static let version: String? = {
        let info = Bundle.main.infoDictionary
        return (info?["CFBundleShortVersionString"] as? String).map { "Kudzu \($0)" }
    }()

    var body: some View {
        KzListHeader("Settings", subtitle: Self.version)
            .padding(.leading, KzWindowChrome.Lights.trailingEdge)
        VStack(alignment: .leading, spacing: 1) {
            KzSidebarItem("Library", icon: .stack) { returnToLibrary() }
            ForEach(SettingsSection.allCases) { section in
                KzSidebarItem(section.title, icon: section.icon, isActive: section == selection) {
                    selection = section
                }
            }
        }
        .padding(.horizontal, KzSpace.s2)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Settings sections")
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
