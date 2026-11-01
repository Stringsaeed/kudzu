import KudzuUI
import SwiftUI

struct TogglePage: GalleryPage {
    static let id = "toggle"
    static let title = "Toggle"
    static let section = GallerySection.components
    static let icon = KzIconName.checkCircle

    @State private var on = true

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Native controls with Kudzu metrics. States below: rest, hover, press, focus, disabled.") {
            SpecimenSection("Interactive") {
                GalleryCard {
                    KzToggle("Expansion", isOn: $on)
                }
            }
            SpecimenSection("Variants and states") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: 16) {
                        ForEach([false, true], id: \.self) { on in
                            HStack(spacing: 20) {
                                KzToggle("Expansion", isOn: .constant(on))
                                KzToggle("Expansion", isOn: .constant(on)).kzPreviewInteraction(hovered: true)
                                KzToggle("Expansion", isOn: .constant(on)).kzPreviewInteraction(pressed: true)
                                KzToggle("Expansion", isOn: .constant(on)).kzPreviewInteraction(focused: true)
                                KzToggle("Expansion", isOn: .constant(on)).disabled(true)
                            }
                        }
                    }
                }
            }
        }
    }
}
