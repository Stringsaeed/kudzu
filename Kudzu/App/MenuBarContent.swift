import AppKit
import SwiftUI

/// Menu shown from the menu bar icon.
struct MenuBarContent: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openWindow) private var openWindow

    private var permissionsMissing: Bool {
        !model.permissions.accessibility || !model.permissions.inputMonitoring
    }

    var body: some View {
        @Bindable var engine = model.engine

        Text(model.engine.statusText)

        if let storeError = model.storeError {
            Text(storeError)
        }

        if permissionsMissing {
            Button {
                openWindow(id: SceneID.onboarding)
                NSApplication.shared.activate()
            } label: {
                Label("Permissions needed — Fix…", systemImage: "exclamationmark.triangle")
            }
        }

        if model.engine.secureInputActive {
            let owner = model.engine.secureInputOwner ?? "another app"
            Text("Secure Input on (\(owner)) — expansion blocked")
            Button("Why?") {
                presentSecureInputAlert(owner: model.engine.secureInputOwner)
            }
        }

        Toggle("Pause Kudzu", isOn: $engine.isPaused)

        Divider()

        Button("Open Kudzu") {
            openWindow(id: SceneID.main)
            NSApplication.shared.activate()
        }

        Button("New Snippet from Selection") {
            Task { @MainActor in
                if let selection = await model.engine.captureSelection(),
                   !selection.isEmpty {
                    model.pendingNewSnippetContent = selection
                    openWindow(id: SceneID.main)
                    NSApplication.shared.activate()
                } else {
                    presentNoSelectionAlert()
                }
            }
        }

        searchSnippetsButton

        Divider()

        SettingsLink {
            Text("Settings…")
        }
        .keyboardShortcut(",", modifiers: .command)

        Divider()

        Button("Quit Kudzu") {
            NSApplication.shared.terminate(nil)
        }
        .keyboardShortcut("q")
    }

    /// Opens the palette, showing its global shortcut (as a key equivalent when SwiftUI can express it).
    @ViewBuilder
    private var searchSnippetsButton: some View {
        let shortcut = model.paletteShortcut
        let registered = model.paletteShortcutError == nil
        if registered, let key = shortcut.keyEquivalent {
            Button("Search Snippets…", action: openPalette)
                .keyboardShortcut(key, modifiers: shortcut.eventModifiers)
        } else if registered {
            Button("Search Snippets…  \(shortcut.displayString)", action: openPalette)
        } else {
            Button("Search Snippets…", action: openPalette)
        }
    }

    private func openPalette() {
        // Run after the menu has finished closing, or it takes key focus back from the panel.
        Task { @MainActor in model.showPalette() }
    }

    @MainActor
    private func presentSecureInputAlert(owner: String?) {
        NSApplication.shared.activate()
        let alert = NSAlert()
        alert.alertStyle = .informational
        alert.messageText = "Secure Input Is Active"
        let appDescription = owner.map { " (\($0))" } ?? ""
        alert.informativeText =
            "A password field, terminal, or security tool\(appDescription) enabled macOS Secure Event Input. " +
            "While Secure Input is active, macOS prevents apps like Kudzu from monitoring keystrokes to protect sensitive text. " +
            "Expansion will resume automatically once you leave the secure field or close the app that enabled it."
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    @MainActor
    private func presentNoSelectionAlert() {
        NSApplication.shared.activate()
        let alert = NSAlert()
        alert.alertStyle = .informational
        alert.messageText = "No Text Selected"
        alert.informativeText = "Select some text in the active application first, then choose “New Snippet from Selection”."
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }
}

/// Menu bar icon; reflects paused, Secure Input and permission state.
struct MenuBarIcon: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openWindow) private var openWindow

    private var permissionsMissing: Bool {
        !model.permissions.accessibility || !model.permissions.inputMonitoring
    }

    private var symbolName: String {
        if permissionsMissing {
            return "exclamationmark.triangle.fill"
        }
        if model.engine.secureInputActive {
            return "lock.fill"
        }
        if model.engine.isPaused {
            return "text.badge.minus"
        }
        return "text.badge.plus"
    }

    var body: some View {
        Image(systemName: symbolName)
            .symbolRenderingMode(.monochrome)
            .opacity(model.engine.isPaused && !permissionsMissing && !model.engine.secureInputActive ? 0.55 : 1.0)
            .accessibilityLabel("Kudzu")
            // The label is the only view built at launch, so it opens the welcome window.
            // (`defaultLaunchBehavior(.presented)` doesn't open windows for a menu-bar-only app.)
            .onAppear {
                model.attach(openWindow: openWindow)
                model.openOnboardingIfNeeded(openWindow)
            }
    }
}
