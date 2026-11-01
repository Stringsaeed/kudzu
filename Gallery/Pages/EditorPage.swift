import KudzuUI
import SwiftUI

struct EditorPage: GalleryPage {
    static let id = "editor"
    static let title = "Editor"
    static let section = GallerySection.components
    static let icon = KzIconName.braces

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "The content editor for a snippet: an Insert menu and a hint bar on top, the user's text below at reading size, with dynamic tags shown inline.")
    }
}
