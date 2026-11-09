import AppKit
import SwiftUI

/// The library window (`tauri.conf.json` `main`): 1180×760 (min 900×560), centered, transparent
/// full-size titlebar. The library toolbar occupies that title row; traffic lights sit at x = 18,
/// centered in it. The chrome is one sidebar-material glass
/// surface and the rounded content card paints over it. Closing hides the window; Kudzu keeps running
/// in the menu bar.
@MainActor
final class MainWindowController: NSWindowController, NSWindowDelegate {
    static let size = NSSize(width: 1180, height: 760)
    static let minSize = NSSize(width: 900, height: 560)
    /// `trafficLightPosition.x` in tauri.conf.json. The lights share the toolbar row.
    static let trafficLightX: CGFloat = 18

    /// Called when the user closes the window (it's hidden, not destroyed).
    var onClose: (() -> Void)?

    init(root: some View, title: String) {
        let window = NSWindow(
            contentRect: NSRect(origin: .zero, size: Self.size),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = title
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.titlebarSeparatorStyle = .none
        window.minSize = Self.minSize
        window.isReleasedWhenClosed = false
        window.isOpaque = false
        window.backgroundColor = .clear
        window.tabbingMode = .disallowed

        let bounds = NSRect(origin: .zero, size: Self.size)
        let hosting = GlassHostingView(rootView: AnyView(root))
        hosting.sizingOptions = [] // the window's size is AppKit's, not the content's ideal size
        // The toolbar draws in the title row. Leaving the titlebar in the safe area would push it
        // onto a second row under the traffic lights.
        hosting.safeAreaRegions = .keyboard
        hosting.frame = bounds
        hosting.autoresizingMask = [.width, .height]
        window.contentView = Self.chrome(hosting: hosting, bounds: bounds)
        window.center()

        super.init(window: window)
        window.delegate = self
        positionTrafficLights()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    var isVisible: Bool { window?.isVisible ?? false }
    var isKey: Bool { window?.isKeyWindow ?? false }

    func show() {
        guard let window else { return }
        if window.isMiniaturized { window.deminiaturize(nil) }
        if !window.styleMask.contains(.fullScreen) {
            let frame = WindowPlacement.recover(window.frame, screens: NSScreen.screens.map(\.visibleFrame))
            if frame != window.frame { window.setFrame(frame, display: false) }
        }
        window.makeKeyAndOrderFront(nil)
        NSApp.activate()
        positionTrafficLights()
    }

    func hide() {
        window?.orderOut(nil)
    }

    // MARK: NSWindowDelegate

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        onClose?()
        return false
    }

    func windowDidResize(_ notification: Notification) { positionTrafficLights() }
    func windowDidExitFullScreen(_ notification: Notification) { positionTrafficLights() }

    // MARK: Traffic lights

    /// Puts the traffic lights on the toolbar row: the titlebar container matches the toolbar
    /// height, and the buttons start at `trafficLightX`, vertically centered.
    func positionTrafficLights() {
        guard let window, !window.styleMask.contains(.fullScreen),
              let close = window.standardWindowButton(.closeButton),
              let mini = window.standardWindowButton(.miniaturizeButton),
              let zoom = window.standardWindowButton(.zoomButton),
              let container = close.superview?.superview else { return }
        let buttonHeight = close.frame.height
        let barHeight = LibraryToolbar.height
        var frame = container.frame
        frame.size.height = barHeight
        frame.origin.y = window.frame.height - barHeight
        if frame != container.frame { container.frame = frame }
        let spacing = mini.frame.minX - close.frame.minX
        for (i, button) in [close, mini, zoom].enumerated() {
            var f = button.frame
            f.origin.x = Self.trafficLightX + CGFloat(i) * spacing
            f.origin.y = (barHeight - buttonHeight) / 2
            if f.origin != button.frame.origin { button.setFrameOrigin(f.origin) }
        }
        // The titlebar's own material would paint a solid band over the window glass.
        uncover(container, keeping: close)
    }

    /// Hides titlebar vibrancy that sits behind the buttons, without hiding the buttons.
    private func uncover(_ view: NSView, keeping button: NSView) {
        for subview in view.subviews {
            if let effect = subview as? NSVisualEffectView, effect !== button.superview, !effect.subviews.contains(where: { owns($0, button) }) {
                effect.isHidden = true
            } else {
                uncover(subview, keeping: button)
            }
        }
    }

    private func owns(_ view: NSView, _ button: NSView) -> Bool {
        if view === button { return true }
        return view.subviews.contains { owns($0, button) }
    }

    /// Glass behind the SwiftUI root: the `sidebar` material Tauri set in `windowEffects`. The
    /// split view's own column backgrounds are switched to this same material (`GlassChrome` in
    /// the library), so the toolbar, sidebar and the margin around the card read as one surface.
    /// The rounded card paints its own solid fill on top.
    static let material: NSVisualEffectView.Material = .sidebar

    private static func chrome(hosting: NSView, bounds: NSRect) -> NSView {
        let effect = NSVisualEffectView()
        effect.material = material
        effect.blendingMode = .behindWindow
        effect.state = .followsWindowActiveState
        effect.frame = bounds
        effect.autoresizingMask = [.width, .height]
        effect.addSubview(hosting)
        return effect
    }
}

/// Keeps reachable windows where the user left them, recovering a hidden window from a removed
/// display only when its titlebar cannot be dragged on any current screen.
enum WindowPlacement {
    static func recover(_ frame: NSRect, screens: [NSRect]) -> NSRect {
        let screens = screens.filter { $0.width > 0 && $0.height > 0 }
        guard !screens.isEmpty else { return frame }
        let titlebar = NSRect(x: frame.minX, y: frame.maxY - 32, width: frame.width, height: 32)
        if screens.contains(where: {
            let visible = titlebar.intersection($0)
            return !visible.isNull && visible.width >= min(120, frame.width, $0.width)
                && visible.height >= min(22, frame.height, $0.height)
        }) { return frame }

        // Recover to the nearest screen. Keep the size even on a display smaller than the
        // window's minimum size, with its top edge and a usable drag region visible.
        guard let screen = screens.min(by: {
            distance(from: frame, to: $0) < distance(from: frame, to: $1)
        }) else { return frame }
        let x = screen.width >= frame.width
            ? min(max(frame.minX, screen.minX), screen.maxX - frame.width)
            : screen.minX
        let y = screen.height >= frame.height
            ? min(max(frame.minY, screen.minY), screen.maxY - frame.height)
            : screen.maxY - frame.height
        return NSRect(origin: NSPoint(x: x, y: y), size: frame.size)
    }

    private static func distance(from frame: NSRect, to screen: NSRect) -> CGFloat {
        let dx = max(screen.minX - frame.midX, frame.midX - screen.maxX, 0)
        let dy = max(screen.minY - frame.midY, frame.midY - screen.maxY, 0)
        return dx * dx + dy * dy
    }
}

/// Hosts the library without painting an opaque window fill over the glass.
private final class GlassHostingView: NSHostingView<AnyView> {
    override var isOpaque: Bool { false }
}
