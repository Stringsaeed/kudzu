import KudzuUI
import SwiftUI

struct BadgePage: GalleryPage {
    static let id = "badge"
    static let title = "Badge"
    static let section = GallerySection.controls
    static let icon = KzIconName.bolt

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A short status label. The word carries the meaning; the tone supports it.") {
            SpecimenSection("Tones") {
                GalleryCard {
                    HStack(spacing: KzSpace.s2) {
                        KzBadge("Manual"); KzBadge("On", tone: .vine); KzBadge("Favorite", tone: .bloom)
                        KzBadge("Overwrites", tone: .warning); KzBadge("Failed", tone: .danger)
                    }
                }
            }
        }
    }
}
