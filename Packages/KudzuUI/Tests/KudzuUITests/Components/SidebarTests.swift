@testable import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct SidebarSnapshotTests {
    /// A 220pt sidebar column: the glass tint over `canvas`, like the real pane in a snapshot.
    private func pane<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: KzSpace.s1) { content() }
            .padding(KzSpace.s2)
            .frame(width: KzSize.sidebar, alignment: .leading)
            .kzGlass(.regular, in: Rectangle(), blending: .behindWindow)
    }

    @Test func itemStates() throws {
        try snapshot("sidebar-item-states", width: 480, background: .canvas) {
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
                    KzSidebarItem("Active, hovered", icon: .folder, count: 15, isActive: true) {}.kzPreviewInteraction(hovered: true)
                    KzSidebarItem("Settings", icon: .sliders, shortcut: "⌘,") {}
                    KzSidebarItem("A very long group name that must truncate", icon: .folder, count: 1234) {}
                }
            }
        }
    }

    @Test func section() throws {
        try snapshot("sidebar-section", width: 480, background: .canvas) {
            pane {
                KzSidebarSection {
                    KzSidebarItem("All snippets", icon: .stack, count: 18, isActive: true) {}
                    KzSidebarItem("Favorites", icon: .star, count: 3) {}
                }
                Color.clear.frame(height: KzSpace.s3)
                KzSidebarSection("Groups", action: { KzIconButton(.plus, label: "New group", size: .sm) {} }) {
                    KzSidebarItem("base", icon: .folder, count: 15) {}
                    KzSidebarItem("Getting started", icon: .folder, count: 3) {}
                }
                Color.clear.frame(height: KzSpace.s3)
                KzSidebarSection("Tags") {
                    KzSidebarItem("Work", icon: .folder) {}
                }
            }
        }
    }
}
