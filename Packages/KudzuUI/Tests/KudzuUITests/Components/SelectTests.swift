import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct SelectSnapshotTests {
    @Test func states() throws {
        try snapshot("select-states", width: 400) {
            VStack(alignment: .leading, spacing: 16) {
                KzSelect("All apps") {}
                KzSelect("All apps") {}.kzPreviewInteraction(hovered: true)
                KzSelect("All apps") {}.kzPreviewInteraction(pressed: true)
                KzSelect("All apps") {}.kzPreviewInteraction(focused: true)
                KzSelect("All apps") {}.disabled(true)
            }.frame(width: 220)
        }
    }
}
