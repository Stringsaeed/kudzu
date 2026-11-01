import KudzuUI
import SwiftUI

struct ButtonPage: GalleryPage {
    static let id = "button"
    static let title = "Button"
    static let section = GallerySection.components
    static let icon = KzIconName.compose


    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Native controls with Kudzu metrics. States below: rest, hover, press, focus, disabled.") {
            SpecimenSection("Interactive") {
                GalleryCard {
                    KzButton("Save", variant: .primary, isDefault: true) {}
                }
            }
            SpecimenSection("Variants and states") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: 16) {
                        ForEach(KzButton.Variant.allCases, id: \.self) { variant in
                            HStack(spacing: 14) {
                                KzButton("Save", variant: variant, icon: .check) {}
                                KzButton("Save", variant: variant) {}.kzPreviewInteraction(hovered: true)
                                KzButton("Save", variant: variant) {}.kzPreviewInteraction(pressed: true)
                                KzButton("Save", variant: variant) {}.kzPreviewInteraction(focused: true)
                                KzButton("Save", variant: variant) {}.disabled(true)
                            }
                            HStack {
                                KzButton("Insert", variant: variant, size: .sm, icon: .plus, iconEnd: .chevronDown) {}
                                KzButton("Insert", variant: variant, size: .sm) {}.kzPreviewInteraction(hovered: true)
                                KzButton("Insert", variant: variant, size: .sm) {}.kzPreviewInteraction(pressed: true)
                                KzButton("Insert", variant: variant, size: .sm) {}.kzPreviewInteraction(focused: true)
                                KzButton("Insert", variant: variant, size: .sm) {}.disabled(true)
                            }
                        }
                    }
                }
            }
        }
    }
}
