import KudzuUI
import SwiftUI

struct SegmentedPage: GalleryPage {
    static let id = "segmented"
    static let title = "Segmented"
    static let section = GallerySection.components
    static let icon = KzIconName.sliders

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A two-to-four option switch for mutually exclusive modes (\"Inherit from group\" / \"Custom\", \"System\" / \"Light\" / \"Dark\").")
    }
}
