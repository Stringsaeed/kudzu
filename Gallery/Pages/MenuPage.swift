import KudzuUI
import SwiftUI

struct MenuPage: GalleryPage {
    static let id = "menu"
    static let title = "Menu"
    static let section = GallerySection.components
    static let icon = KzIconName.chevronsV

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A floating menu (sort, debug, insert tag, select options): a glass panel of items with an optional title and separators.")
    }
}
