import KudzuUI
import SwiftUI

struct ListHeaderPage: GalleryPage {
    static let id = "list-header"
    static let title = "ListHeader"
    static let section = GallerySection.components
    static let icon = KzIconName.sort

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "The header row of the list pane: title, count, and the list's own actions (sort, new snippet).")
    }
}
