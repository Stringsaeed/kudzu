import AppKit
@testable import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct LayoutOverflowTests {
    @Test func settingsRowFitsNarrowDetailPane() {
        let row = KzSettingsRow("Device ID", description: "Usage is counted separately on each Mac.") {
            Text("01234567-89AB-CDEF-0123-456789ABCDEF")
                .kzTextStyle(.monoCaption)
                .lineLimit(1)
                .truncationMode(.middle)
                .frame(maxWidth: 200, alignment: .trailing)
        }
        .frame(width: KzSize.detailMin - KzSpace.s4)

        let host = NSHostingView(rootView: row)
        host.layoutSubtreeIfNeeded()
        #expect(host.fittingSize.width <= KzSize.detailMin)
    }

    @Test func snippetRowFitsListPaneWidth() {
        let row = KzSnippetRow(
            name: "Meeting notes",
            triggers: [";sig", ";signature", ";s"],
            preview: "Best regards",
            group: "Work",
            uses: 12,
            isFavorite: true
        )
        .frame(width: KzSize.list - KzSpace.s4)

        let host = NSHostingView(rootView: row)
        host.layoutSubtreeIfNeeded()
        #expect(host.fittingSize.width <= KzSize.list)
    }
}
