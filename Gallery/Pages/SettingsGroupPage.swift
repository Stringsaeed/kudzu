import KudzuUI
import SwiftUI

struct SettingsGroupPage: GalleryPage {
    static let id = "settings-group"
    static let title = "SettingsGroup"
    static let section = GallerySection.components
    static let icon = KzIconName.stack

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A titled card on the surface page: surface-raised, line border, radius-lg; rows split by line hairlines. Groups stack space-6 apart. Controls are placeholders.") {
            VStack(alignment: .leading, spacing: KzSpace.s6) {
                KzSettingsGroup("Startup", description: "Kudzu runs in the menu bar and expands snippets in every app.") {
                    KzSettingsRow("Launch Kudzu at login", description: "Starts quietly in the menu bar", icon: .keyboard) { PlaceholderToggle(isOn: true) }
                    KzSettingsRow("Show welcome again") { PlaceholderButton(title: "Show…", small: true) }
                }
                KzSettingsGroup("Backups", action: { PlaceholderButton(title: "Back up now", small: true) }) {
                    KzSettingsRow("Keep backups", description: "Keeps the last 14 snapshots.") { PlaceholderSelect(value: "14 snapshots") }
                }
            }
        }
    }
}
