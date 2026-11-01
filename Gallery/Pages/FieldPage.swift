import KudzuUI
import SwiftUI

struct FieldPage: GalleryPage {
    static let id = "field"
    static let title = "Field"
    static let section = GallerySection.components
    static let icon = KzIconName.type

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A labelled block in the snippet editor or a settings page: label on top, control, optional hint below.")
    }
}
