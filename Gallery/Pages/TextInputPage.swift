import KudzuUI
import SwiftUI

struct TextInputPage: GalleryPage {
    static let id = "text-input"
    static let title = "TextInput"
    static let section = GallerySection.components
    static let icon = KzIconName.cursor

    @State private var text = ""

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Native controls with Kudzu metrics. States below: rest, hover, press, focus, disabled.") {
            SpecimenSection("Interactive") {
                GalleryCard {
                    KzTextInput("Snippet name", text: $text).frame(width: 300)
                }
            }
            SpecimenSection("Variants and states") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: 16) {
                        KzTextInput("Snippet name", text: .constant(""))
                        KzTextInput("Snippet name", text: .constant("Address")).kzPreviewInteraction(hovered: true)
                        KzTextInput("Snippet name", text: .constant("Address")).kzPreviewInteraction(pressed: true)
                        KzTextInput("Snippet name", text: .constant("Address")).kzPreviewInteraction(focused: true)
                        KzTextInput("Snippet name", text: .constant("Address")).disabled(true)
                        KzTextInput("Prefix", text: .constant(":addr"), mono: true)
                        KzTextInput("Prefix", text: .constant(":addr"), mono: true).kzPreviewInteraction(focused: true)
                        KzTextInput("Prefix", text: .constant(":addr"), mono: true).kzPreviewInteraction(hovered: true)
                    KzTextInput("Prefix", text: .constant(":addr"), mono: true).kzPreviewInteraction(pressed: true)
                    KzTextInput("Prefix", text: .constant(":addr"), mono: true).disabled(true)
                    }.frame(width: 300)
                }
            }
        }
    }
}
