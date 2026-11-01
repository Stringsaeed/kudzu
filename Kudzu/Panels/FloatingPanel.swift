import AppKit
import SwiftUI

@MainActor
final class FloatingPanel {
    private let panel: NSPanel

    init() {
        panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 320, height: 130),
                        styleMask: [.titled, .nonactivatingPanel, .utilityWindow], backing: .buffered, defer: false)
        panel.title = "Kudzu caret test"
        panel.level = .floating
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
    }

    func show() {
        let location = CaretLocator().locate()
        panel.contentView = NSHostingView(rootView: VStack(alignment: .leading, spacing: 10) {
            Text("Kudzu works! 🌱").font(.headline)
            Text(location.source.rawValue).font(.caption)
            Button("Close") { [weak self] in self?.hide() }
        }.padding().frame(width: 320, height: 130))
        let point = NSPoint(x: location.rect.midX, y: location.rect.midY)
        let screen = NSScreen.screens.first { $0.frame.contains(point) }
            ?? NSScreen.screens.first { $0.frame.contains(NSEvent.mouseLocation) }
            ?? NSScreen.main
        if let frame = screen?.visibleFrame {
            let size = panel.frame.size
            var y = location.rect.minY - size.height - 6
            if y < frame.minY { y = location.rect.maxY + 6 }
            panel.setFrameOrigin(NSPoint(
                x: min(max(location.rect.minX, frame.minX), max(frame.minX, frame.maxX - size.width)),
                y: min(max(y, frame.minY), max(frame.minY, frame.maxY - size.height))))
        }
        panel.orderFrontRegardless()
    }

    func hide() { panel.orderOut(nil) }
}
