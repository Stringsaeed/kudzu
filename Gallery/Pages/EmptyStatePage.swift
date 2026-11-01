import KudzuUI
import SwiftUI

struct EmptyStatePage: GalleryPage {
    static let id = "empty-state"
    static let title = "EmptyState"
    static let section = GallerySection.structure
    static let icon = KzIconName.leaf

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A faint leaf mark, a short title, one sentence that says what to do (with the shortcut), and optionally an action.") {
            SpecimenSection("Variants") {
                GalleryCard {
                    HStack(alignment: .top, spacing: 0) {
                        KzEmptyState("No snippet selected") {
                            KzMessage("Choose a snippet to edit it, or press \(kbd: "⌘N") to create one.")
                        }
                        .frame(maxWidth: .infinity)
                        KzEmptyState("No matches", icon: .search, message: "Try a different search, or clear the filter.") {
                            KzButton("Clear search", size: .sm) {}
                        }
                        .frame(maxWidth: .infinity)
                    }
                }
            }
        }
    }
}
