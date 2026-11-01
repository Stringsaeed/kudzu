import KudzuUI
import SwiftUI

struct SelectPage: GalleryPage {
    static let id = "select"
    static let title = "Select"
    static let section = GallerySection.components
    static let icon = KzIconName.chevronsV


    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Native controls with Kudzu metrics. States below: rest, hover, press, focus, disabled.") {
            SpecimenSection("Interactive") {
                GalleryCard {
                    KzSelect("All apps") {}
                }
            }
            SpecimenSection("Variants and states") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: 16) {
                        KzSelect("All apps") {}
                        KzSelect("All apps") {}.kzPreviewInteraction(hovered: true)
                        KzSelect("All apps") {}.kzPreviewInteraction(pressed: true)
                        KzSelect("All apps") {}.kzPreviewInteraction(focused: true)
                        KzSelect("All apps") {}.disabled(true)
                    }.frame(width: 220)
                }
            }
        }
    }
}
