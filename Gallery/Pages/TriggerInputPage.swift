import KudzuUI
import SwiftUI

struct TriggerInputPage: GalleryPage {
    static let id = "trigger-input"
    static let title = "TriggerInput"
    static let section = GallerySection.components
    static let icon = KzIconName.textPlus

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "The trigger editor: removable TriggerChips plus an inline mono input; space or return adds the draft, backspace on empty removes the last chip.")
    }
}
