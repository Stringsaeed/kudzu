// Native translucency: `NSVisualEffectView` in SwiftUI. The Tauri app asked for the same materials
// through `windows.rs` (`floating_effects`): the main window's sidebar follows the window's active
// state; the palette, clipboard and form panels use the popover material, always active, with a
// rounded mask (18 pt, form 16 pt) so the window shadow is recomputed from the rounded shape.

import AppKit
import SwiftUI

/// `NSVisualEffectView` with material, blending, state, emphasis and a rounded mask.
public struct VisualEffect: NSViewRepresentable {
    /// The native vibrancy material.
    public var material: NSVisualEffectView.Material
    /// Whether the material samples behind or within the window.
    public var blending: NSVisualEffectView.BlendingMode
    /// How the material follows window activation.
    public var state: NSVisualEffectView.State
    /// Whether the native material is emphasized.
    public var emphasized: Bool
    /// The native mask’s corner radius in points.
    public var cornerRadius: CGFloat

    /// Creates VisualEffect with the supplied content and styling.
    public init(
        material: NSVisualEffectView.Material,
        blending: NSVisualEffectView.BlendingMode = .behindWindow,
        state: NSVisualEffectView.State = .followsWindowActiveState,
        emphasized: Bool = false,
        cornerRadius: CGFloat = 0
    ) {
        self.material = material
        self.blending = blending
        self.state = state
        self.emphasized = emphasized
        self.cornerRadius = cornerRadius
    }

    /// The main window's sidebar: `.sidebar`, behind the window, following the active state.
    public static let sidebar = VisualEffect(material: .sidebar, blending: .behindWindow, state: .followsWindowActiveState)

    /// Floating panels (palette, clipboard picker): `.popover`, always active, 18 pt corners.
    public static let popover = VisualEffect(material: .popover, blending: .behindWindow, state: .active, cornerRadius: Radius.palettePanel)

    /// The fill-in form panel: `.popover`, always active, 16 pt corners.
    public static let form = VisualEffect(material: .popover, blending: .behindWindow, state: .active, cornerRadius: Radius.formPanel)

    /// Creates the native visual-effect view.
    public func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        view.wantsLayer = true
        update(view)
        return view
    }

    /// Updates the native material and rounded mask.
    public func updateNSView(_ view: NSVisualEffectView, context: Context) {
        update(view)
    }

    private func update(_ view: NSVisualEffectView) {
        if view.material != material { view.material = material }
        if view.blendingMode != blending { view.blendingMode = blending }
        if view.state != state { view.state = state }
        if view.isEmphasized != emphasized { view.isEmphasized = emphasized }
        view.maskImage = cornerRadius > 0 ? Self.mask(radius: cornerRadius) : nil
        view.window?.invalidateShadow()
    }

    /// A stretchable rounded-rect mask (cap insets keep the corners crisp at any size).
    public static func mask(radius: CGFloat) -> NSImage {
        let edge = radius * 2 + 1
        let image = NSImage(size: NSSize(width: edge, height: edge), flipped: false) { rect in
            NSColor.black.setFill()
            NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius).fill()
            return true
        }
        image.capInsets = NSEdgeInsets(top: radius, left: radius, bottom: radius, right: radius)
        image.resizingMode = .stretch
        return image
    }
}

extension View {
    /// Places a visual-effect material behind this view.
    public func visualEffect(_ effect: VisualEffect) -> some View {
        background(effect.ignoresSafeArea())
    }
}

/// Rounds a borderless window's content and recomputes its shadow from that shape (`round_corners`
/// in `windows.rs`): a transparent `NSWindow`'s shadow is derived from its content's alpha, so a
/// square content view under a rounded vibrancy view would cast square corners.
@MainActor
public enum WindowRounding {
    /// Rounds the window content and updates its native shadow.
    public static func round(_ window: NSWindow, radius: CGFloat) {
        guard let content = window.contentView else { return }
        content.wantsLayer = true
        content.layer?.cornerRadius = radius
        content.layer?.masksToBounds = true
        window.invalidateShadow()
    }
}
