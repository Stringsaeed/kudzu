import AppKit
import KudzuUI
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
            Button("Permissions needed — Fix…") {
                openWindow(id: SceneID.onboarding)
                NSApplication.shared.activate()
            }
        }

        if model.engine.secureInputActive {
            let owner = model.engine.secureInputOwner ?? "another app"
            Text("Secure Input on (\(owner)) — expansion blocked")
            Button("Why…") {
                presentSecureInputAlert(owner: model.engine.secureInputOwner)
            }
        }

        Toggle("Pause Kudzu", isOn: $engine.isPaused)

        Divider()

        Button("Open Kudzu") {
            openWindow(id: SceneID.main)
            NSApplication.shared.activate()
        }

        Button("New snippet from selection") {
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

        Button("Settings…") {
            model.showSettings()
            openWindow(id: SceneID.main)
            NSApplication.shared.activate()
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
            Button("Search snippets…", action: openPalette)
                .keyboardShortcut(key, modifiers: shortcut.eventModifiers)
        } else if registered {
            Button("Search snippets…  \(shortcut.displayString)", action: openPalette)
        } else {
            Button("Search snippets…", action: openPalette)
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
        alert.messageText = "Secure Input is active"
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
        alert.messageText = "No text selected"
        alert.informativeText = "Select some text in the active application first, then choose “New snippet from selection”."
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }
}

/// Menu bar icon; reflects paused, Secure Input and permission state.
struct MenuBarIcon: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openWindow) private var openWindow

    private static let images: [KzMenuBarIcon: NSImage] = {
        var result: [KzMenuBarIcon: NSImage] = [:]
        for state in KzMenuBarIcon.allCases {
            result[state] = state.image()
        }
        return result
    }()

    private var permissionsMissing: Bool {
        !model.permissions.accessibility || !model.permissions.inputMonitoring
    }

    private var icon: KzMenuBarIcon {
        if permissionsMissing {
            return .permissionsMissing
        }
        if model.engine.secureInputActive {
            return .secureInput
        }
        if model.engine.isPaused {
            return .paused
        }
        return .active
    }

    var body: some View {
        let state = icon
        Image(nsImage: Self.images[state] ?? state.image())
            .accessibilityLabel(state.accessibilityDescription)
            // The label is the only view built at launch, so it opens the welcome window.
            // (`defaultLaunchBehavior(.presented)` doesn't open windows for a menu-bar-only app.)
            .onAppear {
                model.attach(openWindow: openWindow)
                model.openOnboardingIfNeeded(openWindow)
            }
    }
}
