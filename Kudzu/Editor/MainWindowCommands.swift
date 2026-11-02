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
}

extension FocusedValues {
    /// The key main window's commands.
    @Entry var mainWindowActions: MainWindowActions?
}

/// The app menu's Settings (⌘,), File › New snippet (⌘N), Edit › Duplicate (⌘D) and View › Back to library (⌘[).
///
/// Settings is a mode of the main window, so ⌘, switches that window into settings and brings it
/// forward, opening it first when it is closed.
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
            Button("New snippet") {
                if let actions { actions.newSnippet() } else { showMainWindow() }
            }
            .keyboardShortcut("n", modifiers: .command)
        }

        CommandGroup(after: .pasteboard) {
            Button("Duplicate") { actions?.duplicate?() }
                .keyboardShortcut("d", modifiers: .command)
                .disabled(actions?.duplicate == nil)
        }

        CommandGroup(before: .sidebar) {
            Button("Back to library") { actions?.leaveSettings?() }
                .keyboardShortcut("[", modifiers: .command)
                .disabled(actions?.leaveSettings == nil)
            Divider()
        }
    }

    private func showMainWindow() {
        openWindow(id: SceneID.main)
        NSApplication.shared.activate()
    }
}
