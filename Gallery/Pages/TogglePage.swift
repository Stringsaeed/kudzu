import KudzuUI
import SwiftUI

struct TogglePage: GalleryPage {
    static let id = "toggle"
    static let title = "Toggle"
    static let section = GallerySection.components
    static let icon = KzIconName.checkCircle

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "An on/off switch for settings that apply immediately.")
    }
}
