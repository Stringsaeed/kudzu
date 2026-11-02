import AppKit
@testable import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct LayoutOverflowTests {
    @Test func longSelectValueStaysInsideItsAvailableWidth() async {
        let measured = MeasuredWidth()
        let label = KzSelectLabel("Default (Type, or the app's setting in Settings → Apps)")
            .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { measured.value = $0 }
            .frame(width: 220)
        let host = NSHostingView(rootView: label)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 220, height: 40),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = host
        host.layoutSubtreeIfNeeded()
        await settle(0.05)
        #expect(measured.value > 0)
        #expect(measured.value <= 220)
    }

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

@MainActor
private final class MeasuredWidth {
    var value: CGFloat = 0
}
