import KudzuUI
import SwiftUI

struct StepperPage: GalleryPage {
    static let id = "stepper"
    static let title = "Stepper"
    static let section = GallerySection.components
    static let icon = KzIconName.plus

    @State private var speed = 40

    var body: some View {
        GalleryPageLayout(title: Self.title, summary: "Native controls with Kudzu metrics. States below: rest, hover, press, focus, disabled.") {
            SpecimenSection("Interactive") {
                GalleryCard {
                    KzStepper("Typing speed", value: $speed, step: 5, min: 10, max: 200, unit: "wpm")
                }
            }
            SpecimenSection("Variants and states") {
                GalleryCard {
                    VStack(alignment: .leading, spacing: 16) {
                        KzStepper("Typing speed", value: .constant(40), step: 5, min: 10, max: 200, unit: "wpm")
                        KzStepper("Typing speed", value: .constant(40), unit: "wpm").kzPreviewInteraction(hovered: true)
                        KzStepper("Typing speed", value: .constant(40), unit: "wpm").kzPreviewInteraction(pressed: true)
                        KzStepper("Typing speed", value: .constant(40), unit: "wpm").kzPreviewInteraction(focused: true)
                        KzStepper("Typing speed", value: .constant(40), unit: "wpm").disabled(true)
                        HStack { KzStepper("Minimum", value: .constant(0), min: 0, max: 10); KzStepper("Maximum", value: .constant(10), min: 0, max: 10) }
                    }
                }
            }
        }
    }
}
