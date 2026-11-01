import KudzuUI
import SwiftUI

struct ToolbarGroupPage: GalleryPage {
    static let id = "toolbar-group"
    static let title = "ToolbarGroup"
    static let section = GallerySection.components
    static let icon = KzIconName.more

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A small glass capsule that floats icon buttons over the detail pane: the one place glass appears inside the content area.")
    }
}
