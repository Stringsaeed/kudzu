import AppKit
import KudzuUI
import SwiftUI

/// Makes the welcome window one calm surface: the window takes the page colour, there's no zoom
/// button (it can't be resized, so a greyed one is just noise), and it can be dragged by its background.
struct WelcomeWindowChrome: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { Probe() }
    func updateNSView(_ nsView: NSView, context: Context) {}

    private final class Probe: NSView {
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            guard let window else { return }
            window.backgroundColor = KzColorToken.surface.nsColor
            window.isMovableByWindowBackground = true
            window.standardWindowButton(.zoomButton)?.isHidden = true
        }
    }
}
