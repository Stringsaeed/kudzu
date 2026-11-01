import KudzuUI
import SwiftUI

struct SidebarItemPage: GalleryPage {
    static let id = "sidebar-item"
    static let title = "SidebarItem"
    static let section = GallerySection.components
    static let icon = KzIconName.sidebar

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Icon, label, optional count or shortcut, on the glass sidebar. Active is a solid glass-selected pill with shadow-control and a vine-ink icon. A real Button with the selected trait.") {
            SpecimenSection("States", note: "Rest, active, hovered, pressed, focused, with a shortcut, and a long label.") {
                HStack(alignment: .top, spacing: KzSpace.s4) {
                    pane {
                        KzSidebarItem("All snippets", icon: .stack, count: 18) {}
                        KzSidebarItem("Favorites", icon: .star, count: 3, isActive: true) {}
                        KzSidebarItem("Recently used", icon: .clock) {}
                        KzSidebarItem("Stats", icon: .chart) {}
                    }
                    pane {
                        KzSidebarItem("Hovered", icon: .folder, count: 15) {}.kzPreviewInteraction(hovered: true)
                        KzSidebarItem("Pressed", icon: .folder, count: 15) {}.kzPreviewInteraction(hovered: true, pressed: true)
                        KzSidebarItem("Focused", icon: .folder, count: 3) {}.kzPreviewInteraction(focused: true)
                        KzSidebarItem("Settings", icon: .sliders, shortcut: "⌘,") {}
                        KzSidebarItem("A very long group name that must truncate", icon: .folder, count: 1234) {}
                    }
                }
            }
        }
    }

    private func pane<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 1) { content() }
            .padding(KzSpace.s2)
            .frame(width: KzSize.sidebar, alignment: .leading)
            .kzGlass(.regular, in: Rectangle(), blending: .withinWindow)
    }
}
