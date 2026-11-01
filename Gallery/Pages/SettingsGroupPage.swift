import KudzuUI
import SwiftUI

struct SettingsGroupPage: GalleryPage {
    static let id = "settings-group"
    static let title = "SettingsGroup"
    static let section = GallerySection.components
    static let icon = KzIconName.stack

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A titled card of SettingsRows on a settings page.")
    }
}
