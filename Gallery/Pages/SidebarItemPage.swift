import KudzuUI
import SwiftUI

struct SidebarItemPage: GalleryPage {
    static let id = "sidebar-item"
    static let title = "SidebarItem"
    static let section = GallerySection.components
    static let icon = KzIconName.sidebar

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "A row in the sidebar (and in the settings section list): icon, label, optional count or shortcut.")
    }
}
