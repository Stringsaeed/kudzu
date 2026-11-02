import KudzuUI
import SwiftUI

struct AppShellPage: GalleryPage {
    static let id = "app-shell"
    static let title = "AppShell"
    static let section = GallerySection.structure
    static let icon = KzIconName.sidebar
    static let snapshotWidth: CGFloat = 1320

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "The one Kudzu window: a glass sidebar (220), the list pane on surface (300), the detail pane on surface-raised. Every header row is 52pt, lined up with the traffic lights. Shown embedded: in a real window `KzAppShell(mode: .window)` also applies KzWindowChrome, so the system draws the lights and the sidebar blur reaches the desktop.") {
            SpecimenSection("Window", note: "Mock traffic lights are drawn at the real positions (x 19, centre y 26). Everything is live outside snapshots: sort, Insert tag, Debug and More open menus, and right-clicking a row opens its context menu.") {
                GallerySampleShell().frame(height: 760)
            }
        }
    }
}
