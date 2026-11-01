import KudzuUI
import SwiftUI

struct TriggerInputPage: GalleryPage {
    static let id = "trigger-input"
    static let title = "TriggerInput"
    static let section = GallerySection.components
    static let icon = KzIconName.textPlus

    @State private var triggers = [":an", ";sig"]

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Native controls with Kudzu metrics. States below: rest, hover, press, focus, disabled.") {
            SpecimenSection("Interactive") {
                GalleryCard {
                    KzTriggerInput(triggers: $triggers) { $0.count > 20 ? "Use at most 20 characters." : nil }.frame(width: 320)
                }
            }
            SpecimenSection("Variants and states") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: 16) {
                        KzTriggerInput(triggers: .constant([]))
                        KzTriggerInput(triggers: .constant([":an", ";signature"]))
                        KzTriggerInput(triggers: .constant([":an", ";signature"])).kzPreviewInteraction(hovered: true)
                        KzTriggerInput(triggers: .constant([":an"])).kzPreviewInteraction(pressed: true)
                        KzTriggerInput(triggers: .constant([":an"])).kzPreviewInteraction(focused: true)
                        KzTriggerInput(triggers: .constant([":an"])).disabled(true)
                        KzTriggerInput(triggers: .constant([":address", ";signature", "//date", ":email", ":phone", ";regards"]))
                    }.frame(width: 320)
                }
            }
        }
    }
}
