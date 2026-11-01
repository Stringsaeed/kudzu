import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct SearchFieldSnapshotTests {
    @Test func states() throws {
        try snapshot("searchfield-states", width: 400) {
            VStack(alignment: .leading, spacing: 16) {
                KzSearchField(text: .constant(""))
                KzSearchField(text: .constant("address")).kzPreviewInteraction(hovered: true)
                KzSearchField(text: .constant("address")).kzPreviewInteraction(pressed: true)
                KzSearchField(text: .constant("address")).kzPreviewInteraction(focused: true)
                KzSearchField(text: .constant("address")).disabled(true)
                KzSearchField(text: .constant(""), shortcut: nil)
            }.frame(width: 300)
        }
    }
}
