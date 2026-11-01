import KudzuUI
import SwiftUI

struct SearchFieldPage: GalleryPage {
    static let id = "search-field"
    static let title = "SearchField"
    static let section = GallerySection.components
    static let icon = KzIconName.search

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "The search field at the top of the list pane; it filters the list below it, so it lives with the list, not in the window toolbar.")
    }
}
