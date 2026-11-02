import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct TriggerInputSnapshotTests {
    @Test func states() throws {
        try snapshot("triggerinput-states", width: 400) {
            VStack(alignment: .leading, spacing: 16) {
                KzTriggerInput(triggers: .constant([]))
                KzTriggerInput(triggers: .constant([":an", ";signature"]))
                KzTriggerInput(triggers: .constant([":an", ";signature"])).kzPreviewInteraction(hovered: true)
                KzTriggerInput(triggers: .constant([":an"])).kzPreviewInteraction(pressed: true)
                KzTriggerInput(triggers: .constant([":an"])).kzPreviewInteraction(focused: true)
                KzTriggerInput(triggers: .constant([":an"])).disabled(true)
                KzTriggerInput(triggers: .constant([":address", ";signature", "//date", ":email", ":phone", ";regards"]))
            }.frame(width: 320)
        }
    }
}
