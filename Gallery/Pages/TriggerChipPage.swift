import KudzuUI
import SwiftUI

struct TriggerChipPage: GalleryPage {
    static let id = "trigger-chip"
    static let title = "TriggerChip"
    static let section = GallerySection.components
    static let icon = KzIconName.braces

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "The monospace chip that shows a trigger: Kudzu's signature element, used everywhere a trigger appears.")
    }
}
