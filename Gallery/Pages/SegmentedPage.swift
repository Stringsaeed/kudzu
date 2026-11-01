import KudzuUI
import SwiftUI

struct SegmentedPage: GalleryPage {
    static let id = "segmented"
    static let title = "Segmented"
    static let section = GallerySection.controls
    static let icon = KzIconName.sliders

    @State private var mode = "System"

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Native controls with Kudzu metrics. States below: rest, hover, press, focus, disabled.") {
            SpecimenSection("Interactive") {
                GalleryCard {
                    KzSegmented("Theme", selection: $mode, options: [.init("System", label: "System"), .init("Light", label: "Light"), .init("Dark", label: "Dark")])
                }
            }
            SpecimenSection("Variants and states") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: 16) {
                        KzSegmented("Theme", selection: .constant("System"), options: [.init("System", label: "System"), .init("Light", label: "Light"), .init("Dark", label: "Dark")])
                        KzSegmented("Mode", selection: .constant("Custom"), options: [.init("Inherit", label: "Inherit from group"), .init("Custom", label: "Custom")]).kzPreviewInteraction(hovered: true)
                        KzSegmented("Mode", selection: .constant("Custom"), options: [.init("Inherit", label: "Inherit"), .init("Custom", label: "Custom")]).kzPreviewInteraction(pressed: true)
                        KzSegmented("Mode", selection: .constant("Custom"), options: [.init("Inherit", label: "Inherit"), .init("Custom", label: "Custom")]).kzPreviewInteraction(focused: true)
                        KzSegmented("Mode", selection: .constant(1), options: [.init(1, label: "One"), .init(2, label: "Two"), .init(3, label: "Three"), .init(4, label: "Four")]).disabled(true)
                    }
                }
            }
        }
    }
}
