import SwiftUI

// Placeholder: owned by the app-shell task (menu bar, onboarding, settings).

/// Menu shown from the menu bar icon.
struct MenuBarContent: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Button("Open Kudzu") { openWindow(id: SceneID.main) }
        Button("Welcome…") { openWindow(id: SceneID.onboarding) }
        Divider()
        Button("Quit Kudzu") { NSApplication.shared.terminate(nil) }
            .keyboardShortcut("q")
    }
}

/// Menu bar icon; reflects paused, Secure Input and permission state.
struct MenuBarIcon: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        Image(systemName: "text.badge.plus")
    }
}
