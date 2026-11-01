import KudzuUI
import SwiftUI

struct ListRowPage: GalleryPage {
    static let id = "list-row"
    static let title = "ListRow"
    static let section = GallerySection.components
    static let icon = KzIconName.list

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A generic two-line list row for non-snippet lists: backups, stats ranking.")
    }
}
