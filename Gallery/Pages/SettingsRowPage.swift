import KudzuUI
import SwiftUI

struct SettingsRowPage: GalleryPage {
    static let id = "settings-row"
    static let title = "SettingsRow"
    static let section = GallerySection.components
    static let icon = KzIconName.sliders

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Label (body-strong) and optional description (callout, ink-muted) and icon on the left; the control on the right. At least 52pt tall. Controls are placeholders.") {
            SpecimenSection("Variants") {
                KzSettingsGroup {
                    KzSettingsRow("Label only") { PlaceholderToggle(isOn: false) }
                    KzSettingsRow("With description", description: "One sentence, no trailing period for fragments") { PlaceholderToggle(isOn: true) }
                    KzSettingsRow("With icon", description: "A 16pt ink-muted glyph", icon: .keyboard) { PlaceholderSelect(value: "Normal") }
                    KzSettingsRow(description: "A custom label view, here a title and a badge", label: {
                        HStack(spacing: KzSpace.s2) { Text("iCloud sync"); KzBadge("Coming soon") }
                    }, control: { PlaceholderToggle(isOn: false) })
                    KzSettingsRow("Segmented control") { PlaceholderSegmented(options: ["Fast", "Normal", "Slow"], selected: 1) }
                }
            }
        }
    }
}
