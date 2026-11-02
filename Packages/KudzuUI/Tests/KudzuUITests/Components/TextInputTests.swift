import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct TextInputSnapshotTests {
    @Test func states() throws {
        try snapshot("textinput-states", width: 400) {
            VStack(alignment: .leading, spacing: 16) {
                KzTextInput("Snippet name", text: .constant(""))
                KzTextInput("Snippet name", text: .constant("Address")).kzPreviewInteraction(hovered: true)
                KzTextInput("Snippet name", text: .constant("Address")).kzPreviewInteraction(pressed: true)
                KzTextInput("Snippet name", text: .constant("Address")).kzPreviewInteraction(focused: true)
                KzTextInput("Snippet name", text: .constant("Address")).disabled(true)
                KzTextInput("Prefix", text: .constant(":addr"), mono: true)
                KzTextInput("Prefix", text: .constant(":addr"), mono: true).kzPreviewInteraction(focused: true)
                KzTextInput("Prefix", text: .constant(":addr"), mono: true).kzPreviewInteraction(hovered: true)
                KzTextInput("Prefix", text: .constant(":addr"), mono: true).kzPreviewInteraction(pressed: true)
                KzTextInput("Prefix", text: .constant(":addr"), mono: true).disabled(true)
            }.frame(width: 300)
        }
    }
}
