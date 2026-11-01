import KudzuUI
import SwiftUI

struct AppShellPage: GalleryPage {
    static let id = "app-shell"
    static let title = "AppShell"
    static let section = GallerySection.components
    static let icon = KzIconName.sidebar

    // Placeholder: replace with specimens of every variant and state (see Packages/KudzuUI/README.md).
    var body: some View {
        ComponentPlaceholder(name: Self.title, summary: "The one Kudzu window: a glass sidebar, the list pane on `surface`, the detail pane on `surface-raised`. Settings is a mode of this window, not a second window.")
    }
}
