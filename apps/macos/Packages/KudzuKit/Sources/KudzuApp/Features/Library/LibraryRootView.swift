// The main window (`main-app.tsx`): toolbar, sidebar, snippet list and the editor slot, or the
// Insights / Clipboard / Settings slot for those routes. Onboarding replaces the chrome until it
// is finished. ⌘K, ⌘/ and group settings are sheets. Toasts (delete undo, import, a ready update)
// sit on the root.
//
// T12 hosts this view. Pass the editor and the other screens through `LibrarySlots`; until then
// each slot shows a placeholder. The view reads `@Environment(AppModel.self)`.

import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

/// The library window. `LibraryScreen` is the same view.
public struct LibraryRootView: View {
    var slots: LibrarySlots

    public init(slots: LibrarySlots = LibrarySlots()) {
        self.slots = slots
    }

    public var body: some View {
        LibraryInstalled(slots: slots)
    }
}

public typealias LibraryScreen = LibraryRootView

private struct LibraryInstalled: View {
    var slots: LibrarySlots
    @Environment(AppModel.self) private var model

    var body: some View {
        LibraryChrome(slots: slots, model: model)
    }
}

private struct LibraryChrome: View {
    var slots: LibrarySlots
    @Environment(AppModel.self) private var model
    @State private var focus: LibraryFocus
    @State private var actions: LibraryActions
    @State private var columns: NavigationSplitViewVisibility = .all
    @State private var announcedUpdate: String?

    init(slots: LibrarySlots, model: AppModel) {
        self.slots = slots
        let focus = LibraryFocus()
        _focus = State(initialValue: focus)
        _actions = State(initialValue: LibraryActions(model: model, focus: focus))
    }

    var body: some View {
        @Bindable var model = model
        @Bindable var actions = actions
        Group {
            if !model.ready {
                Color.kz.background
            } else if !model.settings.onboardingCompleted {
                ZStack(alignment: .topTrailing) {
                    slots.onboarding()
                    if model.isDev { Badge("Dev").padding(16) }
                }
            } else {
                chrome
            }
        }
        .environment(focus)
        .environment(actions)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .sheet(isPresented: $model.commandOpen) {
            CommandMenuSheet()
                .environment(model)
                .environment(actions)
                .environment(focus)
        }
        .sheet(isPresented: $model.shortcutsOpen) {
            ShortcutsSheet()
                .environment(model)
        }
        .sheet(item: $actions.groupSettings) { group in
            GroupSettingsForm(group: group, appPicker: slots.groupAppPicker)
                .environment(model)
                .environment(actions)
        }
        .onChange(of: model.updateState) { _, state in announceUpdate(state) }
        // `background` sits outside the `.environment` modifiers above, so pass them in.
        .background { LibraryCommands().environment(focus).environment(actions) }
        .onKeyPress(phases: .down) { press in
            guard LibraryKeyboardPolicy.opensShortcuts(
                character: press.characters,
                command: press.modifiers.contains(.command),
                control: press.modifiers.contains(.control),
                option: press.modifiers.contains(.option),
                editingText: NSApp.keyWindow?.firstResponder is NSTextView
            ) else { return .ignored }
            model.shortcutsOpen = true
            return .handled
        }
        .kzToaster()
    }

    private var chrome: some View {
        VStack(spacing: 0) {
            LibraryToolbar(columns: $columns)
            // A fixed sidebar with room for aligned counts and keycaps, hidden when closed.
            // Not a NavigationSplitView: on macOS 26 that wraps
            // the sidebar in its own Liquid Glass, tinting it apart from the toolbar and margins.
            HStack(spacing: 0) {
                if columns != .detailOnly {
                    SidebarView()
                        .frame(width: 256)
                }
                detail
                    .padding(.vertical, 8) // `my-2 mr-2` on <main>
                    .padding(.trailing, 8)
                    .padding(.leading, columns == .detailOnly ? 8 : 0)
            }
        }
        // `bg-sidebar` over the glass: the dark (55%) or light (72%) sidebar tint that the web's
        // `.vibrant` theme lays over the window material, so the warm grays read as they do there.
        .background { Rectangle().fill(SidebarFill()) }
        .vibrant()
        // Lists and scroll views paint their own fills over the window glass. Drop them so the
        // toolbar, sidebar and the margin around the card stay one glass surface. The card keeps
        // its fill.
        .background { GlassChrome() }
    }

    @ViewBuilder private var detail: some View {
        let card = RoundedRectangle(cornerRadius: Radius.xl2, style: .continuous)
        Group {
            if model.inLibrary {
                HStack(spacing: 0) {
                    SnippetListView()
                        .frame(minWidth: 260, idealWidth: 300, maxWidth: 320)
                    Hairline(.vertical)
                    editor
                }
            } else if model.view == .insights {
                slots.insights()
            } else if case let .clipboard(filter) = model.view {
                slots.clipboard(filter)
            } else if case let .settings(section) = model.view {
                slots.settings(section)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .clipShape(card)
        .depth(.card, fill: Color.kz.background, in: card)
    }

    @ViewBuilder private var editor: some View {
        if let snippet = model.selectedSnippet {
            slots.editor(snippet)
                .id(snippet.id)
        } else {
            VStack(spacing: 12) {
                Text("No snippet selected")
                    .font(.system(size: 14, weight: .medium))
                Text("Create one and it opens here.")
                    .font(.system(size: 12))
                    .foregroundStyle(Color.kz.mutedForeground)
                Button { actions.newSnippet() } label: { ButtonLabel("New snippet", icon: Icon.plus) }
                    .buttonStyle(.kz(size: .sm))
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    /// "Kudzu x.y is ready", once per version, unless Settings is already showing it.
    private func announceUpdate(_ state: UpdateState) {
        guard case let .ready(version, _, _) = state, version != announcedUpdate else { return }
        announcedUpdate = version
        guard model.view.kind != "settings" else { return }
        let model = model
        toast(
            "Kudzu \(version) is ready",
            description: "It installs when you quit, or restart now.",
            action: ToastButton("Restart") { Task { try? await model.backend.restartToUpdate() } },
            cancel: ToastButton("What's new") { model.setView(.settings(section: .about)) },
            duration: 15
        )
    }
}

/// Clears opaque list and scroll fills, and gives any nested effect view the window's own
/// material. Two behind-window effect views with the same material render identically, so
/// nothing inside the chrome shows a visible edge.
private struct GlassChrome: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { NSView() }
    func updateNSView(_ nsView: NSView, context: Context) {
        DispatchQueue.main.async { clear(nsView.window?.contentView) }
    }

    private func clear(_ view: NSView?) {
        guard let view else { return }
        if let scroll = view as? NSScrollView {
            scroll.drawsBackground = false
            scroll.backgroundColor = .clear
        }
        if let clip = view as? NSClipView {
            clip.drawsBackground = false
        }
        if let effect = view as? NSVisualEffectView {
            effect.material = .sidebar // MainWindowController.material
            effect.blendingMode = .behindWindow
            effect.state = .followsWindowActiveState
        }
        view.subviews.forEach { clear($0) }
    }
}

/// Window shortcuts from `main-app.tsx`. Each action no-ops when the web's `enabled` flag is false.
private struct LibraryCommands: View {
    @Environment(AppModel.self) private var model
    @Environment(LibraryActions.self) private var actions
    @Environment(LibraryFocus.self) private var focus

    var body: some View {
        Group {
            shortcut("Search", key: "k", modifiers: .command) { model.commandOpen = true }
            shortcut("New snippet", key: "n", modifiers: .command) { actions.newSnippet() }
            shortcut("New group", key: "n", modifiers: [.command, .shift]) { actions.newGroup(rename: LibraryGroupCreationSource.windowShortcut.startsRename) }
            shortcut("Filter", key: "f", modifiers: .command) { focusFilter() }
            shortcut("Focus list", key: "l", modifiers: .command) { focusList() }
            shortcut("Edit", key: "e", modifiers: .command) {
                if model.inLibrary, model.selectedId != nil { focus.focusEditor() }
            }
            shortcut("All snippets", key: "1", modifiers: .command) { model.setView(.all) }
            shortcut("Favorites", key: "2", modifiers: .command) { model.setView(.favorites) }
            shortcut("Recently used", key: "3", modifiers: .command) { model.setView(.recent) }
            shortcut("Insights", key: "4", modifiers: .command) { model.setView(.insights) }
            shortcut("Clipboard", key: "5", modifiers: .command) { model.setView(.clipboard(filter: nil)) }
            shortcut("Settings", key: ",", modifiers: .command) { model.setView(.settings(section: nil)) }
            shortcut("Shortcuts", key: "/", modifiers: .command) { model.shortcutsOpen = true }
            shortcut("Pause", key: "p", modifiers: [.command, .shift]) { actions.togglePaused() }
            shortcut("Duplicate", key: "d", modifiers: .command) {
                if model.inLibrary, let id = model.selectedId { actions.duplicate(id) }
            }
            shortcut("Favorite", key: "f", modifiers: [.command, .shift]) {
                if model.inLibrary, let id = model.selectedId { actions.toggleFavorite(id) }
            }
            shortcut("Turn on or off", key: "e", modifiers: [.command, .shift]) {
                if model.inLibrary, let id = model.selectedId { actions.toggleEnabled(id) }
            }
            Button("Delete") {
                if LibraryKeyboardPolicy.canDeleteSelection(
                    inLibrary: model.inLibrary,
                    editingText: NSApp.keyWindow?.firstResponder is NSTextView
                ) { actions.deleteSelection() }
            }
            .keyboardShortcut(.delete, modifiers: .command)
            // Off the snippet routes, ⌘⌫ belongs to the screen showing (the clipboard list).
            .disabled(!model.inLibrary)
        }
        .opacity(0)
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }

    private func shortcut(
        _ title: String,
        key: KeyEquivalent,
        modifiers: EventModifiers,
        action: @escaping () -> Void
    ) -> some View {
        Button(title, action: action).keyboardShortcut(key, modifiers: modifiers)
    }

    private func focusFilter() {
        if model.view.kind == "clipboard" {
            focus.focus(.clipboardSearch)
            return
        }
        if !model.inLibrary { model.setView(.all) }
        focus.focusFilter()
    }

    private func focusList() {
        if model.view.kind == "clipboard" { focus.focus(.clipboardList) } else { focus.focusList() }
    }
}

#Preview("Library") {
    LibraryPreviewHost()
}

#Preview("Library dark") {
    LibraryPreviewHost().preferredColorScheme(.dark)
}

#Preview("Empty library") {
    LibraryPreviewHost(empty: true)
}

#Preview("Shortcuts") {
    LibraryPreviewHost { model in
        ShortcutsSheet().environment(model).frame(width: 720)
    }
}

#Preview("Command menu") {
    LibraryPreviewHost { model in
        CommandMenuSheet()
            .environment(model)
            .environment(LibraryActions(model: model, focus: LibraryFocus()))
    }
}

#Preview("Group settings") {
    LibraryPreviewHost { model in
        if let group = model.library.groups.first {
            GroupSettingsForm(group: group, appPicker: LibrarySlots().groupAppPicker)
                .environment(model)
                .environment(LibraryActions(model: model, focus: LibraryFocus()))
        }
    }
}

private struct LibraryPreviewHost<Content: View>: View {
    var empty = false
    var content: (@MainActor (AppModel) -> Content)?
    @State private var model: AppModel?

    init(empty: Bool = false) where Content == LibraryRootView {
        self.empty = empty
        self.content = nil
    }

    init(@ViewBuilder content: @escaping @MainActor (AppModel) -> Content) {
        self.content = content
    }

    var body: some View {
        Group {
            if let model {
                if let content {
                    content(model)
                } else {
                    LibraryRootView().environment(model)
                }
            } else {
                Color.kz.background
            }
        }
        .frame(width: content == nil ? 1180 : nil, height: content == nil ? 760 : nil)
        .task {
            guard model == nil else { return }
            let next = AppModel(backend: MockBackend(), isDev: true)
            await next.start()
            next.platform = .macos
            if empty {
                next.library = .empty
                next.usage = [:]
                next.clipboard.entries = []
                next.selectedId = nil
            }
            model = next
        }
    }
}
