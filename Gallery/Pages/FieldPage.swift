import KudzuUI
import SwiftUI

struct FieldPage: GalleryPage {
    static let id = "field"
    static let title = "Field"
    static let section = GallerySection.structure
    static let icon = KzIconName.type

    @State private var triggers = [":an"]
    @State private var group = "base"
    @State private var name = ""

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Label (headline), control, optional aside and hint (callout, ink-muted). Fields stack space-5 apart. The label is linked to the control for VoiceOver.") {
            SpecimenSection("Stack") {
                GalleryCard {
                    KzFieldStack {
                        KzField("Triggers", aside: "Space or return to add") { KzTriggerInput(triggers: $triggers) }
                        KzField("Group", hint: "Snippets in a group share its rules.") {
                            KzSelect("Group", selection: $group, options: ["base", "Getting started"].map { ($0, $0) })
                        }
                        KzField("Name") { KzTextInput("Untitled snippet", text: $name) }
                    }
                }
            }
        }
    }
}
