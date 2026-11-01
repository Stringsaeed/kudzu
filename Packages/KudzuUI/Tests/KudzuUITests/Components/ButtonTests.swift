import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct ButtonSnapshotTests {
    @Test func states() throws {
        try snapshot("button-states", width: 600) {
            VStack(alignment: .leading, spacing: 16) {
                ForEach(KzButton.Variant.allCases, id: \.self) { variant in
                    HStack(spacing: 14) {
                        KzButton("Save", variant: variant, icon: .check) {}
                        KzButton("Save", variant: variant) {}.kzPreviewInteraction(hovered: true)
                        KzButton("Save", variant: variant) {}.kzPreviewInteraction(pressed: true)
                        KzButton("Save", variant: variant) {}.kzPreviewInteraction(focused: true)
                        KzButton("Save", variant: variant) {}.disabled(true)
                    }
                    HStack {
                        KzButton("Insert", variant: variant, size: .sm, icon: .plus, iconEnd: .chevronDown) {}
                        KzButton("Insert", variant: variant, size: .sm) {}.kzPreviewInteraction(hovered: true)
                        KzButton("Insert", variant: variant, size: .sm) {}.kzPreviewInteraction(pressed: true)
                        KzButton("Insert", variant: variant, size: .sm) {}.kzPreviewInteraction(focused: true)
                        KzButton("Insert", variant: variant, size: .sm) {}.disabled(true)
                    }
                }
            }
        }
    }
}
