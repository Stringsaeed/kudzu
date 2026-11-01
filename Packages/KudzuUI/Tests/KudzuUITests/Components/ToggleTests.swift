import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct ToggleSnapshotTests {
    @Test func states() throws {
        try snapshot("toggle-states", width: 400) {
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
