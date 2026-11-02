import AppKit
import SwiftUI

/// Window chrome for Kudzu's one window: no title, **no toolbar**, traffic lights kept, content drawn
/// edge to edge on an opaque `surface-raised` window; the sidebar's behind-window blur still reaches the desktop.
///
/// ## Traffic-light alignment
///
/// Every pane header is `size-titlebar` (52pt) tall and must line up with the traffic lights. Without
/// a toolbar AppKit builds a 32pt titlebar with the lights centred 16pt from the top, so the chrome
/// lays them out itself: it grows the titlebar container to 52pt and puts the 14pt buttons at
/// y = 19…33, whose centre (26) is `size-titlebar / 2`, with the close button at x = 19.
///
/// It re-applies that layout whenever AppKit may have redone the titlebar: resizes, full-screen exit,
/// key and main changes, screen and backing changes, and frame changes of the titlebar container.
///
/// An earlier version got the 52pt titlebar from an *empty unified toolbar*. On macOS 26 a toolbar is
/// a Liquid Glass layer drawn over the whole titlebar strip, which blurred the pane headers beneath
/// it, so there must be no toolbar at all.
///
/// The window's own content then needs `ignoresSafeArea(.top)`, which `KzAppShell` does.
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
        // No toolbar: on macOS 26 it is a glass layer over the titlebar that blurs the pane headers.
        if window.toolbar?.identifier == toolbarIdentifier { window.toolbar = nil }
        // An opaque fill. The sidebar's `.behindWindow` effect still blurs the desktop (as in Finder), and
        // anything macOS fades under the titlebar (the macOS 26 scroll edge effect) fades into the
        // window colour instead of showing the desktop through a see-through strip.
        window.isOpaque = true
        window.backgroundColor = KzColorToken.surfaceRaised.nsColor
        layoutTrafficLights(in: window)
    }

    /// Puts the traffic lights in a 52pt titlebar, centred on the pane headers. Safe to call often.
    @MainActor
    public static func layoutTrafficLights(in window: NSWindow) {
        guard !window.styleMask.contains(.fullScreen),
              let close = window.standardWindowButton(.closeButton),
              let titlebar = close.superview, let container = titlebar.superview else { return }
        let height = KzSize.titlebar
        let windowHeight = window.frame.height
        var frame = container.frame
        if frame.height != height || frame.minY != windowHeight - height {
            frame.size.height = height
            frame.origin.y = windowHeight - height
            container.frame = frame
            container.layoutSubtreeIfNeeded()
        }
        let types: [NSWindow.ButtonType] = [.closeButton, .miniaturizeButton, .zoomButton]
        for (index, type) in types.enumerated() {
            guard let button = window.standardWindowButton(type), let parent = button.superview else { continue }
            let size = button.frame.size
            // Window coordinates have their origin at the bottom-left.
            let target = NSRect(x: Lights.leading + CGFloat(index) * Lights.pitch,
                                y: windowHeight - Lights.top - size.height,
                                width: size.width, height: size.height)
            let origin = parent.convert(target, from: nil).origin
            if button.frame.origin != origin { button.setFrameOrigin(origin) }
        }
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
        private var observers: [NSObjectProtocol] = []
        private var isLayingOut = false

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            removeObservers()
            guard let window else { return }
            KzWindowChrome.apply(to: window)
            observe(window)
            // SwiftUI finishes configuring its window (and AppKit its titlebar) after this view moves
            // in, which can put the lights back; lay them out again once that has settled.
            DispatchQueue.main.async { [weak self] in self?.relayout() }
        }

        private func observe(_ window: NSWindow) {
            let center = NotificationCenter.default
            let names: [Notification.Name] = [
                NSWindow.didResizeNotification, NSWindow.didEndLiveResizeNotification,
                NSWindow.didExitFullScreenNotification, NSWindow.didBecomeKeyNotification,
                NSWindow.didResignKeyNotification, NSWindow.didBecomeMainNotification,
                NSWindow.didResignMainNotification, NSWindow.didChangeScreenNotification,
                NSWindow.didChangeBackingPropertiesNotification,
            ]
            for name in names {
                observers.append(center.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                    MainActor.assumeIsolated { self?.relayout() }
                })
            }
            if let container = window.standardWindowButton(.closeButton)?.superview?.superview {
                container.postsFrameChangedNotifications = true
                observers.append(center.addObserver(forName: NSView.frameDidChangeNotification, object: container,
                                                    queue: .main) { [weak self] _ in
                    MainActor.assumeIsolated { self?.relayout() }
                })
            }
        }

        private func relayout() {
            // Setting the container's frame posts a frame change; don't recurse into it.
            guard !isLayingOut, let window else { return }
            isLayingOut = true
            KzWindowChrome.layoutTrafficLights(in: window)
            isLayingOut = false
        }

        private func removeObservers() {
            observers.forEach(NotificationCenter.default.removeObserver)
            observers.removeAll()
        }

        isolated deinit { removeObservers() }
    }
}
