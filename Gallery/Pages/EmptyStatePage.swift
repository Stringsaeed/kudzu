import KudzuUI
import SwiftUI

struct EmptyStatePage: GalleryPage {
    static let id = "empty-state"
    static let title = "EmptyState"
    static let section = GallerySection.components
    static let icon = KzIconName.leaf

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "What a pane shows when it has nothing to show: a faint leaf mark, a short title, one sentence that says what to do, and the shortcut.")
    }
}
