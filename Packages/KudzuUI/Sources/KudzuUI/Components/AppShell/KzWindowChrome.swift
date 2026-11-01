import AppKit
import SwiftUI

/// Window chrome for Kudzu's one window: no title, no toolbar, traffic lights kept, content drawn
/// edge to edge and a clear window background, so the sidebar's behind-window blur reaches the desktop.
///
/// ## Traffic-light alignment
///
/// Every pane header is `size-titlebar` (52pt) tall and must line up with the traffic lights, which
/// AppKit places inside the titlebar. The numbers below were measured on macOS 27.2 by building an
/// offscreen `NSWindow` (`KzWindowChromeTests` re-checks them on whatever OS runs the tests):
///
/// | Window configuration                    | Titlebar height | Lights' centre from top |
/// | --------------------------------------- | --------------- | ----------------------- |
/// | full-size content, no toolbar           | 32              | 16                      |
/// | + empty `.unified` toolbar              | **52**          | **26**                  |
/// | + empty `.unifiedCompact` toolbar       | 40              | 20                      |
/// | + empty `.expanded` toolbar             | 48              | 16                      |
///
/// With the lights 14pt tall (y = 19…33) in a 52pt titlebar, their centre is 19 + 7 = 26 =
/// `size-titlebar / 2`, so a 52pt header row centres its content on the lights exactly.
///
/// Instead of moving the buttons by hand (which AppKit undoes on every resize, full-screen change
/// and appearance change), the chrome gives the window an *empty* unified toolbar. That makes
/// AppKit itself build the 52pt titlebar and centre the lights in it, so the alignment survives
/// anything AppKit does. Horizontally the lights span x = 19…79 (close at 19, minimise at 42, zoom
/// at 65), which is why the sidebar's header slot sits at the trailing edge.
///
/// The window's own content then needs `ignoresSafeArea(.top)` (the 52pt titlebar would otherwise
/// become a safe-area inset), which `KzAppShell` does.
public enum KzWindowChrome {
    /// The toolbar identifier of the empty spacer toolbar.
    public static let toolbarIdentifier = "sh.saeed.kudzu.titlebar"

    /// Where the traffic lights sit in a 52pt titlebar, in points from the window's top-left.
    public enum Lights {
        /// Diameter of each button.
        public static let size: CGFloat = 14
        /// Distance from the top edge to the buttons' top.
        public static let top: CGFloat = 19
        /// x of the close button's left edge.
        public static let leading: CGFloat = 19
        /// Distance between the left edges of adjacent buttons.
        public static let pitch: CGFloat = 23
        /// Vertical centre, which equals `size-titlebar / 2`.
        public static var centerY: CGFloat { top + size / 2 }
        /// x of the zoom button's right edge: the left edge of the free space beside the lights.
        public static var trailingEdge: CGFloat { leading + 2 * pitch + size }
    }

    /// Configures `window` for the Kudzu look. Safe to call repeatedly.
    @MainActor
    public static func apply(to window: NSWindow) {
        window.styleMask.insert(.fullSizeContentView)
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.titlebarSeparatorStyle = .none
        // An empty toolbar is what makes AppKit build the 52pt titlebar. Respect a toolbar the app already set.
        if window.toolbar == nil {
            let toolbar = NSToolbar(identifier: toolbarIdentifier)
            toolbar.displayMode = .iconOnly
            toolbar.allowsUserCustomization = false
            toolbar.autosavesConfiguration = false
            window.toolbar = toolbar
        }
        window.toolbarStyle = .unified
        // No fill: the sidebar's `.behindWindow` visual effect needs a transparent window to blur the desktop.
        window.isOpaque = false
        window.backgroundColor = .clear
        // A transparent window computes its shadow from what is drawn, which isn't settled yet.
        DispatchQueue.main.async { [weak window] in window?.invalidateShadow() }
    }

    /// The close button's vertical centre measured from the top of the window, or `nil` without a close button.
    @MainActor
    public static func closeButtonCenterY(in window: NSWindow) -> CGFloat? {
        guard let button = window.standardWindowButton(.closeButton) else { return nil }
        let frame = button.convert(button.bounds, to: nil)
        return window.frame.height - frame.midY
    }
}

extension View {
    /// Applies `KzWindowChrome` to the window that hosts this view.
    public func kzWindowChrome() -> some View {
        background { KzWindowChromeView().frame(width: 0, height: 0).accessibilityHidden(true) }
    }
}

private struct KzWindowChromeView: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { Probe() }
    func updateNSView(_ view: NSView, context: Context) {}

    final class Probe: NSView {
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            guard let window else { return }
            KzWindowChrome.apply(to: window)
        }
    }
}
