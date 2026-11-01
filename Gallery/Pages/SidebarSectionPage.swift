import KudzuUI
import SwiftUI

struct SidebarSectionPage: GalleryPage {
    static let id = "sidebar-section"
    static let title = "SidebarSection"
    static let section = GallerySection.components
    static let icon = KzIconName.folder

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A titled block of SidebarItems (\"Groups\"), with an optional action at the right of its heading (the + for a new group).")
    }
}
