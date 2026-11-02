import KudzuUI
import SwiftUI

struct SettingsGroupPage: GalleryPage {
    static let id = "settings-group"
    static let title = "SettingsGroup"
    static let section = GallerySection.structure
    static let icon = KzIconName.stack

    @State private var launchAtLogin = true
    @State private var keep = 14

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A titled card on the surface page: surface-raised, line border, radius-lg; rows split by line hairlines. Groups stack space-6 apart.") {
            VStack(alignment: .leading, spacing: KzSpace.s6) {
                KzSettingsGroup("Startup", description: "Kudzu runs in the menu bar and expands snippets in every app.") {
                    KzSettingsRow("Launch Kudzu at login", description: "Starts quietly in the menu bar", icon: .keyboard) { KzToggle("Launch Kudzu at login", isOn: $launchAtLogin) }
                    KzSettingsRow("Show welcome again") { KzButton("Show…", size: .sm) {} }
                }
                KzSettingsGroup("Backups", action: { KzButton("Back up now", size: .sm) {} }) {
                    KzSettingsRow("Keep backups", description: "Keeps the last \(keep) snapshots.") {
                        KzSelect("Keep backups", selection: $keep, options: [7, 14, 30].map { ($0, "\($0) snapshots") })
                    }
                }
            }
        }
    }
}
