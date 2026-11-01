import KudzuUI
import SwiftUI

struct SnippetRowPage: GalleryPage {
    static let id = "snippet-row"
    static let title = "SnippetRow"
    static let section = GallerySection.components
    static let icon = KzIconName.list

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "One snippet in the list pane: name, favorite star, group and use count on top; trigger chips and a one-line preview of the content below.")
    }
}
