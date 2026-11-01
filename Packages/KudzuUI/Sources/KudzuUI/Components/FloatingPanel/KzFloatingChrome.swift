import AppKit
import SwiftUI

/// Geometry shared by every floating layer that lives in its own window (menus, popovers, panels).
///
/// The `shadow-float` / `shadow-glass` drop is drawn by SwiftUI like the CSS, not by the window
/// server, so the window is larger than its glass by these margins. The margins come from the
/// shadow tokens, so they follow the design system if the shadows change.
@MainActor
enum KzFloatingChrome {
    /// Room a shadow token needs around a shape: the furthest any drop layer reaches on each side.
    static func margins(for shadow: KzShadow?) -> NSEdgeInsets {
        guard let shadow else { return NSEdgeInsets() }
        var top: CGFloat = 0, bottom: CGFloat = 0, side: CGFloat = 0
        for layer in shadow.lightLayers + shadow.darkLayers where !layer.inset {
            let reach = layer.blur + layer.spread
            top = max(top, reach - layer.y)
            bottom = max(bottom, reach + layer.y)
            side = max(side, reach + abs(layer.x))
        }
        // A few points of slack so the blur's tail isn't cut off.
        return NSEdgeInsets(top: ceil(top) + 4, left: ceil(side) + 4, bottom: ceil(bottom) + 4, right: ceil(side) + 4)
    }

    /// The window frame for a glass frame (both in screen coordinates).
    static func windowFrame(for chrome: CGRect, margins: NSEdgeInsets) -> CGRect {
        CGRect(x: chrome.minX - margins.left, y: chrome.minY - margins.bottom,
               width: chrome.width + margins.left + margins.right,
               height: chrome.height + margins.top + margins.bottom)
    }

    /// The visible frame of the screen that contains `point`, or the main screen's.
    static func visibleFrame(containing point: CGPoint) -> CGRect {
        let screen = NSScreen.screens.first { NSMouseInRect(point, $0.frame, false) } ?? NSScreen.main
        return screen?.visibleFrame ?? CGRect(x: 0, y: 0, width: 1440, height: 900)
    }

    /// True when the user asked for reduced motion.
    static var reduceMotion: Bool { NSWorkspace.shared.accessibilityDisplayShouldReduceMotion }

    /// Fade duration for floating layers: quick, and none under Reduce Motion.
    static let fadeDuration: TimeInterval = 0.12

    /// Fades a window in (or shows it at once under Reduce Motion).
    static func fadeIn(_ window: NSWindow) {
        if reduceMotion {
            window.alphaValue = 1
            return
        }
        window.alphaValue = 0
        NSAnimationContext.runAnimationGroup { context in
            context.duration = fadeDuration
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            window.animator().alphaValue = 1
        }
    }

    /// Fades a window out and orders it out (at once under Reduce Motion).
    static func fadeOut(_ window: NSWindow, completion: (@MainActor () -> Void)? = nil) {
        window.ignoresMouseEvents = true
        let finish: @MainActor () -> Void = {
            window.orderOut(nil)
            window.alphaValue = 1
            window.ignoresMouseEvents = false
            completion?()
        }
        if reduceMotion || !window.isVisible {
            finish()
            return
        }
        NSAnimationContext.runAnimationGroup { context in
            context.duration = fadeDuration
            context.timingFunction = CAMediaTimingFunction(name: .easeIn)
            window.animator().alphaValue = 0
        } completionHandler: {
            MainActor.assumeIsolated { finish() }
        }
    }
}

/// An invisible AppKit view placed behind a SwiftUI view so floating layers can find the view's
/// frame on screen and its window. It never takes clicks.
final class KzAnchorNSView: NSView {
    /// Mirrors SwiftUI's `isEnabled` for the view it sits behind.
    var isEnabled = true
    /// Called when the view moves to a window or leaves it.
    var windowDidChange: ((NSWindow?) -> Void)?

    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override func isAccessibilityElement() -> Bool { false }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        windowDidChange?(window)
    }

    /// The view's frame in screen coordinates.
    var screenFrame: CGRect? {
        guard let window else { return nil }
        return window.convertToScreen(convert(bounds, to: nil))
    }

    /// True when `event` (a mouse event in this view's window) lands on the visible part of the view.
    func contains(_ event: NSEvent) -> Bool {
        guard let window, event.window === window, !isHiddenOrHasHiddenAncestor else { return false }
        let point = convert(event.locationInWindow, from: nil)
        return visibleRect.contains(point)
    }
}

/// Puts a `KzAnchorNSView` behind a SwiftUI view and keeps its enabled state current.
struct KzAnchorRepresentable: NSViewRepresentable {
    let view: KzAnchorNSView
    var isEnabled: Bool = true
    var onUpdate: (@MainActor () -> Void)?

    func makeNSView(context: Context) -> KzAnchorNSView { view }

    func updateNSView(_ nsView: KzAnchorNSView, context: Context) {
        nsView.isEnabled = isEnabled
        onUpdate?()
    }
}

/// The anchor as a background; left out when rendering offscreen, where AppKit views can't draw.
struct KzAnchorBackground: View {
    let view: KzAnchorNSView
    var isEnabled: Bool = true
    var onUpdate: (@MainActor () -> Void)?
    @Environment(\.kzSnapshotRendering) private var snapshot

    var body: some View {
        if snapshot {
            Color.clear
        } else {
            KzAnchorRepresentable(view: view, isEnabled: isEnabled, onUpdate: onUpdate)
        }
    }
}

/// The window that had key focus and its first responder, so they can be given back.
@MainActor
struct KzFocusMemory {
    weak var window: NSWindow?
    weak var responder: NSResponder?

    /// Remembers the current key window (if it's ours) and its first responder.
    static func capture() -> KzFocusMemory {
        var memory = KzFocusMemory()
        guard let window = NSApp.keyWindow else { return memory }
        memory.window = window
        var responder = window.firstResponder
        // A text field's shared field editor stands in for the field: remember the field.
        if let editor = responder as? NSTextView, editor.isFieldEditor, let field = editor.delegate as? NSResponder {
            responder = field
        }
        memory.responder = responder
        return memory
    }

    /// Makes the window key again and puts focus back where it was, if it moved.
    func restore() {
        guard let window, window.isVisible else { return }
        if !window.isKeyWindow { window.makeKey() }
        guard let responder, Self.responder(responder, belongsTo: window) else { return }
        let current = window.firstResponder
        if current === responder { return }
        if let editor = current as? NSTextView, editor.isFieldEditor, editor.delegate === responder { return }
        window.makeFirstResponder(responder)
    }

    static func responder(_ responder: NSResponder, belongsTo window: NSWindow) -> Bool {
        if responder === window { return true }
        if let view = responder as? NSView { return view.window === window }
        return false
    }
}
