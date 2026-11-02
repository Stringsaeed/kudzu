import KudzuUI
import SwiftUI

struct DisclosurePage: GalleryPage {
    static let id = "disclosure"
    static let title = "Disclosure"
    static let section = GallerySection.structure
    static let icon = KzIconName.chevronRight

    @State private var rules = Rules.inherit
    @State private var insertion = "Inherit"

    private enum Rules: Hashable { case inherit, custom }

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "A chevron and a headline title, an optional one-line summary while closed, and content indented 18px. The header is a real button: Space or Return toggles it, and VoiceOver reads expanded or collapsed.") {
            SpecimenSection("States") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: KzSpace.s4) {
                        KzDisclosure("Rules", isInitiallyExpanded: true) {
                            HStack(spacing: KzSpace.s3) {
                                KzSegmented("Rules", selection: $rules, options: [.init(.inherit, label: "Inherit from group"), .init(.custom, label: "Custom")])
                                Text("Works in every app").kzTextStyle(.callout).foregroundStyle(KzColor.inkMuted)
                            }
                            HStack(spacing: KzSpace.s3) {
                                Text("Insertion").kzTextStyle(.bodyStrong).foregroundStyle(KzColor.ink).frame(width: 72, alignment: .leading)
                                KzSelect("Insertion", selection: $insertion, options: ["Inherit", "Type", "Paste"].map { ($0, $0) })
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
