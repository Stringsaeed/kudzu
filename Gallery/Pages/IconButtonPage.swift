import KudzuUI
import SwiftUI

struct IconButtonPage: GalleryPage {
    static let id = "icon-button"
    static let title = "IconButton"
    static let section = GallerySection.components
    static let icon = KzIconName.more

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A square, borderless button holding one icon, for toolbars and pane headers.")
    }
}
