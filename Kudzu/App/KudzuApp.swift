import SwiftUI

@main
struct KudzuApp: App {
    @State private var spike = SpikeController()

    var body: some Scene {
        MenuBarExtra("Kudzu", systemImage: spike.secureInput.isEnabled ? "lock.fill" : "text.badge.plus") {
            #if APPSTORE
            Text("Kudzu · App Store (sandboxed)")
            #else
            Text("Kudzu · Direct")
            #endif
            ForEach(Permissions.Kind.allCases) { kind in
                Menu("\(kind.rawValue): \(spike.permissions.granted(kind) ? "Granted" : "Not granted")") {
                    Button("Grant…") { spike.permissions.request(kind) }
                    Button("Open System Settings…") { spike.permissions.openSettings(kind) }
                }
            }
            Text(spike.eventTap.status)
            Text(spike.secureInput.isEnabled
                 ? "Secure Input: On (\(spike.secureInput.responsibleApp ?? "owner unknown"))" : "Secure Input: Off")
            Toggle("Pause", isOn: $spike.paused)
            Divider()
            Button("Show panel at caret (⌃⌥Space)") {
                // Let the menu dismiss before querying the previously focused app.
                Task { try? await Task.sleep(for: .milliseconds(150)); spike.showPanel() }
            }
            if spike.hotKeyStatus != 0 { Text("Hotkey registration failed: \(spike.hotKeyStatus)") }
            Text(spike.lastResult)
            Divider()
            Button("Quit Kudzu") { spike.stop(); NSApplication.shared.terminate(nil) }
                .keyboardShortcut("q")
        }
    }
}
