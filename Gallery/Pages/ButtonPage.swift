import KudzuUI
import SwiftUI

struct ButtonPage: GalleryPage {
    static let id = "button"
    static let title = "Button"
    static let section = GallerySection.components
    static let icon = KzIconName.compose

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "Text buttons for commands: one `primary` per view at most, `secondary` for the rest, `ghost` for low-stakes or tertiary actions.")
    }
}
