import AppKit
import SwiftUI

/// What the key main window can do from the menu bar. Published with `focusedSceneValue`, so it is
/// nil when the main window is closed or another window is key.
struct MainWindowActions {
    /// Creates a snippet (and leaves settings).
    var newSnippet: () -> Void
    /// Switches to settings, at the section the window showed last.
    var openSettings: () -> Void
    /// Duplicates the selected snippets; nil when nothing is selected.
    var duplicate: (() -> Void)?
    /// Leaves settings for the library; nil in library mode.
    var leaveSettings: (() -> Void)?
    /// Focuses the search field and selects the query.
    var focusSearch: () -> Void
    /// Focuses the sidebar.
    var focusSidebar: () -> Void
    /// Focuses the snippet list.
    var focusResults: () -> Void
    /// Focuses the editor, selecting a visible snippet first when none is selected.
    var focusEditor: () -> Void
    /// Clears the search and moves to the results. Nil when the query is already empty.
    var clearSearch: (() -> Void)?
    /// Throws away unsaved edits. Nil when the editor is clean. No shortcut: Escape must not discard.
    var discardChanges: (() -> Void)?
    /// Commits the active editor and pending trigger entry.
    var saveChanges: (() -> Void)?
    var copyExpanded: (() -> Void)?
    var copySource: (() -> Void)?
    /// Toggles the favorite on the draft when the editor is open, otherwise on the stored snippets.
    var toggleFavorite: (() -> Void)?
    /// Asks to delete the selection. Nil when nothing is selected.
    var deleteSelection: (() -> Void)?
}

extension FocusedValues {
    /// The key main window's commands.
    @Entry var mainWindowActions: MainWindowActions?
}

/// The app menu commands for the library window.
///
/// Settings is a mode of the main window, so ⌘, switches that window into settings and brings it
/// forward, opening it first when it is closed. ⌘N and the pane commands do the same: a closed
/// window opens, then the command runs. Find is the only search shortcut. Favorite, delete, discard,
/// and the pane moves are in the menus without extra shortcuts.
struct KudzuCommands: Commands {
    let model: AppModel
    @Environment(\.openWindow) private var openWindow
    @FocusedValue(\.mainWindowActions) private var actions

    var body: some Commands {
        CommandGroup(replacing: .appSettings) {
            Button("Settings…") {
                if let actions {
                    actions.openSettings()
                } else {
                    // The window reads the mode when it appears; a hidden or closed one is brought back.
                    if case .settings = model.mainWindowMode {} else { model.showSettings() }
                    showMainWindow()
                }
            }
            .keyboardShortcut(",", modifiers: .command)
        }

        CommandGroup(replacing: .newItem) {
            Button("New snippet") { run(.newSnippet) { $0.newSnippet() } }
                .keyboardShortcut("n", modifiers: .command)
                .disabled(settingsOwnsLibraryCommands)
        }

        CommandGroup(after: .textEditing) {
            Button("Find snippets") { run(.focusSearch) { $0.focusSearch() } }
                .keyboardShortcut("f", modifiers: .command)
                .disabled(settingsOwnsLibraryCommands)
        }

        CommandGroup(replacing: .saveItem) {
            Button("Save changes") { actions?.saveChanges?() }
                .keyboardShortcut("s", modifiers: .command)
                .disabled(actions?.saveChanges == nil || settingsOwnsLibraryCommands)
        }

        CommandGroup(after: .pasteboard) {
            Button("Duplicate") { actions?.duplicate?() }
                .keyboardShortcut("d", modifiers: .command)
                .disabled(actions?.duplicate == nil || settingsOwnsLibraryCommands)
            Button("Discard changes") { actions?.discardChanges?() }
                .disabled(actions?.discardChanges == nil || settingsOwnsLibraryCommands)
        }

        CommandMenu("Snippet") {
            Button("Copy expanded text") { actions?.copyExpanded?() }
                .keyboardShortcut(.return, modifiers: .command)
                .disabled(actions?.copyExpanded == nil || settingsOwnsLibraryCommands)
            Button("Copy template source") { actions?.copySource?() }
                .disabled(actions?.copySource == nil || settingsOwnsLibraryCommands)
            Divider()
            Button("Favorite") { actions?.toggleFavorite?() }
                .disabled(actions?.toggleFavorite == nil)
            Button("Delete…") { actions?.deleteSelection?() }
                .disabled(actions?.deleteSelection == nil)
        }

        CommandGroup(before: .sidebar) {
            Button("Back to library") { actions?.leaveSettings?() }
                .keyboardShortcut("[", modifiers: .command)
                .disabled(actions?.leaveSettings == nil)
            Divider()
            Button("Sidebar") { run(.focusSidebar) { $0.focusSidebar() } }.disabled(settingsOwnsLibraryCommands)
            Button("Snippet list") { run(.focusResults) { $0.focusResults() } }.disabled(settingsOwnsLibraryCommands)
            Button("Editor") { run(.focusEditor) { $0.focusEditor() } }.disabled(settingsOwnsLibraryCommands)
            Divider()
            Button("Clear search") { actions?.clearSearch?() }
                .disabled(actions?.clearSearch == nil || settingsOwnsLibraryCommands)
        }
    }

    /// The key window is showing settings, or a visible main window is. A closed window is not.
    private var settingsOwnsLibraryCommands: Bool {
        guard case .settings = model.mainWindowMode else { return false }
        if actions != nil { return true }
        return NSApp.windows.contains { $0.identifier?.rawValue == SceneID.main && $0.isVisible }
    }

    /// Runs the command on the key window, or opens the window and leaves the request for it.
    private func run(_ request: MainWindowRequest, _ action: (MainWindowActions) -> Void) {
        if let actions {
            action(actions)
        } else {
            model.mainWindowRequest = request
            showMainWindow()
        }
    }

    private func showMainWindow() {
        openWindow(id: SceneID.main)
        NSApplication.shared.activate()
    }
}
