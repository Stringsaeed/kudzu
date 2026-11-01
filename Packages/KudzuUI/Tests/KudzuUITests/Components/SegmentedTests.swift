@testable import KudzuUI
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

@MainActor
@Suite struct SegmentedBehaviorTests {
    /// ← and → change the selection directly and stop at the ends.
    @Test func arrowsMoveTheSelection() {
        let values = ["System", "Light", "Dark"]
        #expect(KzSegmented<String>.moved("System", by: 1, in: values) == "Light")
        #expect(KzSegmented<String>.moved("Light", by: -1, in: values) == "System")
        #expect(KzSegmented<String>.moved("Dark", by: 1, in: values) == "Dark")
        #expect(KzSegmented<String>.moved("System", by: -1, in: values) == "System")
        #expect(KzSegmented<String>.moved("Missing", by: 1, in: values) == "Missing")
    }
}
