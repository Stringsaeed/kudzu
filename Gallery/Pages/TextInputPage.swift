import KudzuUI
import SwiftUI

struct TextInputPage: GalleryPage {
    static let id = "text-input"
    static let title = "TextInput"
    static let section = GallerySection.components
    static let icon = KzIconName.cursor

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A single-line text field: a `surface-sunken` well at rest, `surface-raised` with a 2px `vine-ink` edge on focus.")
    }
}
