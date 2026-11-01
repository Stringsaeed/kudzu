import KudzuUI
import SwiftUI

struct UsageBarPage: GalleryPage {
    static let id = "usage-bar"
    static let title = "UsageBar"
    static let section = GallerySection.components
    static let icon = KzIconName.chart

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A ranked usage bar for \"Most used\": trigger chip, a thin `vine-ink` bar on a `surface-sunken` track, and the count.")
    }
}
