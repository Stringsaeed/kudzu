import KudzuUI
import SwiftUI

@main
struct KudzuApp: App {
    @State private var model: AppModel

    init() {
        // Geist and Geist Mono for KudzuUI; idempotent, and KudzuUI also registers lazily.
        KzFonts.register()
        #if DEBUG
        // `-KudzuSnapshot <dir>` renders the main window offscreen and exits before anything starts.
        MainWindowSnapshots.runIfRequested()
        #endif
        _model = State(initialValue: AppModel())
    }

    var body: some Scene {
        MenuBarExtra {
            MenuBarContent()
                .environment(model)
        } label: {
            MenuBarIcon()
                .environment(model)
        }

        // Settings is a mode of this window (⌘,), not a separate scene.
        Window("Kudzu", id: SceneID.main) {
            MainWindowView()
                .environment(model)
        }
        .defaultSize(width: 1080, height: 700)
        .commands { KudzuCommands(model: model) }

        Window("Welcome to Kudzu", id: SceneID.onboarding) {
            OnboardingView()
                .environment(model)
                .background(DockPolicyAccessor())
        }
        .windowStyle(.hiddenTitleBar)
        .windowResizability(.contentSize)
        .restorationBehavior(.disabled)
    }
}
