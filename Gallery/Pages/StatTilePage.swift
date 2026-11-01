import KudzuUI
import SwiftUI

struct StatTilePage: GalleryPage {
    static let id = "stat-tile"
    static let title = "StatTile"
    static let section = GallerySection.components
    static let icon = KzIconName.chart

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A single headline number on the Stats view.")
    }
}
