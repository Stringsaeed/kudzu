import KudzuUI
import SwiftUI

struct SettingsRowPage: GalleryPage {
    static let id = "settings-row"
    static let title = "SettingsRow"
    static let section = GallerySection.structure
    static let icon = KzIconName.sliders

    @State private var off = false
    @State private var on = true
    @State private var speed = "Normal"
    @State private var pace = Pace.normal

    private enum Pace: Hashable { case fast, normal, slow }

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Label (body-strong) and optional description (callout, ink-muted) and icon on the left; the control on the right. At least 52pt tall.") {
            SpecimenSection("Variants") {
                KzSettingsGroup {
                    KzSettingsRow("Label only") { KzToggle("Label only", isOn: $off) }
                    KzSettingsRow("With description", description: "One sentence, no trailing period for fragments") { KzToggle("With description", isOn: $on) }
                    KzSettingsRow("With icon", description: "A 16pt ink-muted glyph", icon: .keyboard) {
                        KzSelect("Typing speed", selection: $speed, options: ["Slow", "Normal", "Fast"].map { ($0, $0) })
                    }
                    KzSettingsRow(description: "A custom label view, here a title and a badge", label: {
                        HStack(spacing: KzSpace.s2) { Text("iCloud sync"); KzBadge("Coming soon") }
                    }, control: { KzToggle("iCloud sync", isOn: $off) })
                    KzSettingsRow("Segmented control") {
                        KzSegmented("Pace", selection: $pace, options: [.init(.fast, label: "Fast"), .init(.normal, label: "Normal"), .init(.slow, label: "Slow")])
                    }
                }
            }
        }
    }
}
