import AppKit
import KudzuCore

/// Where panels open (`windows.rs`). The Tauri code worked in top-left-origin logical points
/// (like the Accessibility caret rect); these helpers do the same maths and convert to AppKit's
/// bottom-left-origin frames at the end.
enum PanelPlacement {
    // Sizes from `windows.rs` / `permissions/mod.rs`.
    static let paletteSize = NSSize(width: 760, height: 480)
    static let clipboardSize = NSSize(width: 800, height: 520)
    static let formWidth: CGFloat = 440
    static let formHeight: CGFloat = 360
    static let formHeightRange: ClosedRange<CGFloat> = 160 ... 680
    static let permissionsSize = NSSize(width: 420, height: 660)
    static let paletteRadius: CGFloat = 18
    static let formRadius: CGFloat = 16

    /// A monitor as `(x, y, w, h)` with a top-left origin.
    struct Monitor: Equatable {
        var x: CGFloat, y: CGFloat, w: CGFloat, h: CGFloat
    }

    /// The height of the primary screen, which anchors both coordinate systems.
    static var primaryHeight: CGFloat { NSScreen.screens.first?.frame.height ?? 0 }

    /// The monitor under the mouse (else the first one), top-left origin.
    static func activeMonitor() -> Monitor? {
        let mouse = NSEvent.mouseLocation
        guard let screen = NSScreen.screens.first(where: { NSMouseInRect(mouse, $0.frame, false) }) ?? NSScreen.screens.first else { return nil }
        let f = screen.frame
        return Monitor(x: f.minX, y: primaryHeight - f.maxY, w: f.width, h: f.height)
    }

    /// Top-left `(x, y)` of a `size` window to AppKit's frame origin.
    static func appKitOrigin(x: CGFloat, y: CGFloat, height: CGFloat) -> NSPoint {
        NSPoint(x: x, y: primaryHeight - y - height)
    }

    /// Centered horizontally, its top at `fraction` of the monitor's height (palette 20 %,
    /// clipboard 18 %). Returns the top-left point.
    static func centered(width: CGFloat, on m: Monitor, topFraction fraction: CGFloat) -> (x: CGFloat, y: CGFloat) {
        (m.x + (m.w - width) / 2, m.y + m.h * fraction)
    }

    /// `show_form`: just below the caret (or above it when there's no room), clamped to the
    /// monitor; without a caret, centered at 28 % height. Returns the top-left point.
    static func form(caret: ScreenRect?, monitor: Monitor?, height h: CGFloat) -> (x: CGFloat, y: CGFloat) {
        switch (caret, monitor) {
        case let (c?, m?):
            var x = CGFloat(c.x) - 24
            var y = CGFloat(c.y + c.height) + 6
            if y + h > m.y + m.h {
                y = max(CGFloat(c.y) - h - 6, m.y)
            }
            x = min(max(x, m.x + 8), m.x + m.w - formWidth - 8)
            return (x, y)
        case let (nil, m?):
            return (m.x + (m.w - formWidth) / 2, m.y + m.h * 0.28)
        default:
            return (200, 200)
        }
    }
}
