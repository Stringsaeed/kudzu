import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct StepperSnapshotTests {
    @Test func states() throws {
        try snapshot("stepper-states", width: 400) {
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
