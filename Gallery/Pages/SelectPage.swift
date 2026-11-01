import KudzuUI
import SwiftUI

struct SelectPage: GalleryPage {
    static let id = "select"
    static let title = "Select"
    static let section = GallerySection.components
    static let icon = KzIconName.chevronsV

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A pop-up button that shows the current choice and opens a Menu of options.")
    }
}
