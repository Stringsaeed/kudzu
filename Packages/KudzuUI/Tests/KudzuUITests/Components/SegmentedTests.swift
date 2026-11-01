import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct SegmentedSnapshotTests {
    @Test func states() throws {
        try snapshot("segmented-states", width: 400) {
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
