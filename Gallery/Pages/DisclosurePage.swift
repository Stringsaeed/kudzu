import KudzuUI
import SwiftUI

struct DisclosurePage: GalleryPage {
    static let id = "disclosure"
    static let title = "Disclosure"
    static let section = GallerySection.components
    static let icon = KzIconName.chevronRight

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A chevron and a headline title, an optional one-line summary while closed, and content indented 18px. The header is a real button: Space or Return toggles it, and VoiceOver reads expanded or collapsed.") {
            SpecimenSection("States") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: KzSpace.s4) {
                        KzDisclosure("Rules", isInitiallyExpanded: true) {
                            HStack(spacing: KzSpace.s3) {
                                PlaceholderSegmented(options: ["Inherit from group", "Custom"])
                                Text("Works in every app").kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                            }
                            HStack(spacing: KzSpace.s3) {
                                Text("Insertion").kzTextStyle(.bodyStrong).foregroundStyle(KzColor.ink).frame(width: 72, alignment: .leading)
                                PlaceholderSelect(value: "Inherit")
                            }
                        }
                        KzDisclosure("Advanced", summary: "Word boundary, case, undo") { Text("Shown when open") }
                        KzDisclosure("Keyboard focus", summary: "Focus ring") { Text("x") }
                            .kzPreviewInteraction(focused: true)
                    }
                }
            }
        }
    }
}
