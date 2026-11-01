import KudzuUI
import SwiftUI

struct BadgePage: GalleryPage {
    static let id = "badge"
    static let title = "Badge"
    static let section = GallerySection.components
    static let icon = KzIconName.bolt

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A short status label: backup kind, \"Coming soon\", a favorites marker.")
    }
}
