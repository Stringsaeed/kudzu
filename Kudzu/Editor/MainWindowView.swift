import KudzuCore
import SwiftUI

/// Three-pane main window: sidebar, snippet list, editor.
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
    // Column widths aren't stored here: the split view autosaves them natively.
    @State private var started = false

    init(model: AppModel) {
        self.model = model
        _library = State(initialValue: LibraryModel(store: model.store))
    }

    var body: some View {
        NavigationSplitView {
            SidebarView()
                .navigationSplitViewColumnWidth(min: 190, ideal: 220, max: 320)
        } content: {
            contentColumn
                .navigationSplitViewColumnWidth(min: 280, ideal: 330, max: 480)
        } detail: {
            detailColumn
        }
        .frame(minWidth: 940, minHeight: 540)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    browser.newSnippet(library: library)
                } label: {
                    Label("New Snippet", systemImage: "square.and.pencil")
                }
                .keyboardShortcut("n", modifiers: .command)
                .help("New snippet (⌘N)")
            }
            #if DEBUG
            ToolbarItem(placement: .automatic) { DebugMenu() }
            #endif
        }
        .safeAreaInset(edge: .top, spacing: 0) { banners }
        .background(DockPolicyAccessor())
        .environment(library)
        .environment(browser)
        .task { await start() }
        .onChange(of: browser.scope) { _, scope in
            scopeRaw = scope.rawValue
            // Keep the editor in step with the list: drop selected snippets the new scope doesn't show.
            let visible = Set(library.snippets(in: scope, query: "", sort: .name).map(\.id))
            if scope != .stats { browser.selection.formIntersection(visible) }
        }
        .onChange(of: browser.selection) { _, selection in
            selectionRaw = selection.count == 1 ? (selection.first?.uuidString ?? "") : ""
        }
        .onChange(of: library.revision) { browser.prune(using: library) }
        .onChange(of: model.pendingNewSnippetContent) { if started { consumePendingContent() } }
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
        await library.observeChanges()
    }

    /// "New Snippet from Selection" hands its text over through `AppModel`.
    private func consumePendingContent() {
        guard let content = model.pendingNewSnippetContent else { return }
        model.pendingNewSnippetContent = nil
        browser.newSnippet(
            library: library,
            content: content,
            nameHint: UniqueName.name(fromContent: content, fallback: "New Snippet"),
            focus: .trigger
        )
    }

    // MARK: - Columns

    @ViewBuilder
    private var contentColumn: some View {
        if browser.scope == .stats {
            StatsLeaderboardView()
        } else {
            SnippetListView()
        }
    }

    @ViewBuilder
    private var detailColumn: some View {
        if browser.scope == .stats {
            StatsView()
        } else if browser.selection.count == 1, let id = browser.selection.first, let snippet = library.snippetsByID[id] {
            SnippetEditorView(snippet: snippet, library: library)
                .id(id)
        } else if browser.selection.count > 1 {
            ContentUnavailableView(
                "\(browser.selection.count) Snippets Selected",
                systemImage: "square.stack",
                description: Text("Right-click to move, favorite or delete them together, or press Delete.")
            )
        } else {
            ContentUnavailableView(
                "No Snippet Selected",
                systemImage: "text.cursor",
                description: Text("Choose a snippet to edit it, or press ⌘N to create one.")
            )
        }
    }

    // MARK: - Banners

    @ViewBuilder
    private var banners: some View {
        VStack(spacing: 0) {
            if let storeError = model.storeError {
                Banner(
                    message: storeError + " Changes will not be saved.",
                    systemImage: "exclamationmark.octagon.fill",
                    tint: .red,
                    dismiss: nil
                )
            }
            if let message = library.errorMessage {
                Banner(message: message, systemImage: "exclamationmark.triangle.fill", tint: .orange) {
                    library.errorMessage = nil
                }
            }
        }
    }
}

/// A full-width notice at the top of the window.
private struct Banner: View {
    var message: String
    var systemImage: String
    var tint: Color
    var dismiss: (() -> Void)?

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Image(systemName: systemImage)
            Text(message)
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
            Spacer(minLength: 8)
            if let dismiss {
                Button("Dismiss", action: dismiss)
                    .buttonStyle(.borderless)
            }
        }
        .font(.callout)
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(tint.opacity(0.18))
        .overlay(alignment: .bottom) { Rectangle().fill(tint.opacity(0.4)).frame(height: 1) }
        .foregroundStyle(tint == .red ? Color.red : Color.primary)
        .accessibilityElement(children: .combine)
    }
}
