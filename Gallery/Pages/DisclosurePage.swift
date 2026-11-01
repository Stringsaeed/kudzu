import KudzuUI
import SwiftUI

struct DisclosurePage: GalleryPage {
    static let id = "disclosure"
    static let title = "Disclosure"
    static let section = GallerySection.components
    static let icon = KzIconName.chevronRight

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A collapsible section in the snippet editor (Rules, Advanced): a chevron and a `headline` title, with an optional one-line summary shown while closed.")
    }
}
