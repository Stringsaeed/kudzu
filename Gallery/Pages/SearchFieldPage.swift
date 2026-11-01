import KudzuUI
import SwiftUI

struct SearchFieldPage: GalleryPage {
    static let id = "search-field"
    static let title = "SearchField"
    static let section = GallerySection.components
    static let icon = KzIconName.search

    @State private var text = ""
    @FocusState private var searchFocused: Bool

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Native controls with Kudzu metrics. States below: rest, hover, press, focus, disabled.") {
            SpecimenSection("Interactive") {
                GalleryCard {
                    VStack { KzSearchField(text: $text, focus: $searchFocused); KzButton("Find") { searchFocused = true }.keyboardShortcut("f", modifiers: .command) }.frame(width: 300)
                }
            }
            SpecimenSection("Variants and states") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: 16) {
                        KzSearchField(text: .constant(""))
                        KzSearchField(text: .constant("address")).kzPreviewInteraction(hovered: true)
                        KzSearchField(text: .constant("address")).kzPreviewInteraction(pressed: true)
                        KzSearchField(text: .constant("address")).kzPreviewInteraction(focused: true)
                        KzSearchField(text: .constant("address")).disabled(true)
                        KzSearchField(text: .constant(""), shortcut: nil)
                    }.frame(width: 300)
                }
            }
        }
    }
}
