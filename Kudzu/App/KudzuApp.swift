import SwiftUI

@main
struct KudzuApp: App {
    @State private var model = AppModel()

    var body: some Scene {
        MenuBarExtra {
            MenuBarContent()
                .environment(model)
        } label: {
            MenuBarIcon()
                .environment(model)
        }

        Window("Kudzu", id: SceneID.main) {
            MainWindowView()
                .environment(model)
        }
        .defaultSize(width: 980, height: 640)

        Window("Welcome to Kudzu", id: SceneID.onboarding) {
            OnboardingView()
                .environment(model)
                .background(DockPolicyAccessor())
        }
        .windowResizability(.contentSize)
        .restorationBehavior(.disabled)

        Settings {
            SettingsView()
                .environment(model)
        }
    }
}
