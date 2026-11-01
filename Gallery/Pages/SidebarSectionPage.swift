import KudzuUI
import SwiftUI

struct SidebarSectionPage: GalleryPage {
    static let id = "sidebar-section"
    static let title = "SidebarSection"
    static let section = GallerySection.structure
    static let icon = KzIconName.folder

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A titled block of SidebarItems with an optional action at the right of its heading. The heading is caption in ink-muted, sentence case, never uppercase. The + is a small IconButton.") {
            SpecimenSection("Sections") {
                VStack(alignment: .leading, spacing: KzSpace.s4) {
                    KzSidebarSection {
                        KzSidebarItem("All snippets", icon: .stack, count: 18, isActive: true) {}
                        KzSidebarItem("Favorites", icon: .star, count: 3) {}
                    }
                    KzSidebarSection("Groups", action: { KzIconButton(.plus, label: "New group", size: .sm) {} }) {
                        KzSidebarItem("base", icon: .folder, count: 15) {}
                        KzSidebarItem("Getting started", icon: .folder, count: 3) {}
                    }
                }
                .padding(KzSpace.s2)
                .frame(width: KzSize.sidebar, alignment: .leading)
                .kzGlass(.regular, in: Rectangle(), blending: .withinWindow)
            }
        }
    }
}
