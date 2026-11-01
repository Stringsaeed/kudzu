import KudzuUI
import SwiftUI

struct FieldPage: GalleryPage {
    static let id = "field"
    static let title = "Field"
    static let section = GallerySection.components
    static let icon = KzIconName.type

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Label (headline), control, optional aside and hint (callout, ink-muted). Fields stack space-5 apart. The label is linked to the control for VoiceOver.") {
            SpecimenSection("Stack") {
                GalleryCard {
                    KzFieldStack {
                        KzField("Triggers", aside: "Space or return to add") { PlaceholderField(trigger: ":an") }
                        KzField("Group", hint: "Snippets in a group share its rules.") { PlaceholderSelect(value: "base") }
                        KzField("Name") { PlaceholderField(placeholder: "Untitled snippet") }
                    }
                }
            }
        }
    }
}
