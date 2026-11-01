import KudzuUI
import SwiftUI

struct KbdPage: GalleryPage {
    static let id = "kbd"
    static let title = "Kbd"
    static let section = GallerySection.components
    static let icon = KzIconName.keyboard

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A keycap for shortcuts in hints, menus and empty states.")
    }
}
