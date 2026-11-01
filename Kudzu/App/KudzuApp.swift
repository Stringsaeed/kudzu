import KudzuCore
import SwiftUI

@main
struct KudzuApp: App {
    var body: some Scene {
        MenuBarExtra("Kudzu", systemImage: "text.badge.plus") {
            Button("Quit Kudzu") { NSApplication.shared.terminate(nil) }
                .keyboardShortcut("q")
        }
    }
}
