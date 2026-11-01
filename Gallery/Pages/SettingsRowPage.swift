import KudzuUI
import SwiftUI

struct SettingsRowPage: GalleryPage {
    static let id = "settings-row"
    static let title = "SettingsRow"
    static let section = GallerySection.components
    static let icon = KzIconName.sliders

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "One setting: label and optional description on the left, its control on the right. Rows inside a SettingsGroup are split by `line` hairlines.")
    }
}
