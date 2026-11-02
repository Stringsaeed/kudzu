import KudzuUI
import SwiftUI

@main
struct KudzuApp: App {
    @NSApplicationDelegateAdaptor(KudzuAppDelegate.self) private var appDelegate
    @State private var model: AppModel

    init() {
        // Geist and Geist Mono for KudzuUI; idempotent, and KudzuUI also registers lazily.
        KzFonts.register()
        #if DEBUG
        // `-KudzuSnapshot <dir>` renders the main window offscreen and exits before anything starts.
        MainWindowSnapshots.runIfRequested()
        #endif
        let model = AppModel()
        _model = State(initialValue: model)
        appDelegate.prepareForTermination = { model.prepareForTermination() }
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
        // Keeps the titlebar transparent. Without it SwiftUI turns `titlebarAppearsTransparent` back off
        // and macOS draws an opaque titlebar over the top of the pane headers.
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1080, height: 700)
        .windowResizability(.contentMinSize)
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
