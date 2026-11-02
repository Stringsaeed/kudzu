import AppKit
import SwiftUI

/// The two glass tints.
public enum KzGlassStyle: Sendable, CaseIterable {
    /// `glass` (84%): the sidebar.
    case regular
    /// `glass-strong` (92%): menus, popovers and the floating toolbar.
    case strong

    /// The tint token.
    public var tint: KzColorToken {
        switch self {
        case .regular: .glass
        case .strong: .glassStrong
        }
    }
}

/// What the glass blurs.
public enum KzGlassBlending: Sendable {
    /// The desktop behind the window: the sidebar.
    case behindWindow
    /// Window content beneath the layer: floating toolbars, menus and popovers.
    case withinWindow
}

/// A glass layer, following the design system's recipe: system blur, a translucent tint that
/// carries the contrast, a 1px `glass-stroke` highlight inside the top edge and a 1px
/// `glass-edge` outside. Never stack glass on glass.
///
/// Reduce Transparency swaps blur and tint for the opaque tint colour. When rendering
/// offscreen (`\.kzSnapshotRendering`) the system blur can't draw, so only the tint shows.
public struct KzGlass<S: InsettableShape>: View {
    private let style: KzGlassStyle
    private let shape: S
    private let blending: KzGlassBlending

    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.kzSnapshotRendering) private var snapshot

    /// Creates a glass background; use it with `.background { KzGlass(...) }` or via `.kzGlass(...)`.
    public init(_ style: KzGlassStyle = .regular, in shape: S, blending: KzGlassBlending = .withinWindow) {
        self.style = style
        self.shape = shape
        self.blending = blending
    }

    public var body: some View {
        ZStack {
            if reduceTransparency {
                shape.fill(style.tint.opaqueColor)
            } else {
                if !snapshot {
                    KzVisualEffect(blending: blending, style: style)
                        .clipShape(shape)
                }
                shape.fill(style.tint.color)
            }
        }
        .overlay {
            // 1px highlight inside the top edge: the band between the shape and itself moved down 1px.
            KzGlassHighlight(shape: shape)
        }
        .overlay {
            // 1px edge just outside the shape.
            shape.inset(by: -0.5).stroke(KzColor.glassEdge, lineWidth: 1)
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// The 1px `glass-stroke` line along the inside top edge, following the corners.
private struct KzGlassHighlight<S: Shape>: View {
    let shape: S

    var body: some View {
        Canvas { context, size in
            let path = shape.path(in: CGRect(origin: .zero, size: size))
            context.clip(to: path)
            var band = path
            band.addPath(path.offsetBy(dx: 0, dy: 1))
            context.fill(band, with: .color(KzColor.glassStroke), style: FillStyle(eoFill: true))
        }
    }
}

extension View {
    /// Puts the view on a glass layer shaped like `shape`, with an optional shadow token.
    ///
    /// The sidebar uses `.regular` with `.behindWindow`; floating layers use `.strong`,
    /// `.withinWindow` and `shadow: .glass` (toolbar) or `.float` (menus, popovers).
    public func kzGlass<S: InsettableShape>(_ style: KzGlassStyle = .regular, in shape: S, blending: KzGlassBlending = .withinWindow, shadow: KzShadow? = nil) -> some View {
        modifier(KzGlassModifier(style: style, shape: shape, blending: blending, shadow: shadow))
    }
}

private struct KzGlassModifier<S: InsettableShape>: ViewModifier {
    let style: KzGlassStyle
    let shape: S
    let blending: KzGlassBlending
    let shadow: KzShadow?
    @Environment(\.colorScheme) private var colorScheme

    func body(content: Content) -> some View {
        content
            .background {
                ZStack {
                    if let shadow {
                        KzDropShadowView(layers: shadow.layers(for: colorScheme).filter { !$0.inset }, shape: shape)
                    }
                    KzGlass(style, in: shape, blending: blending)
                }
                .allowsHitTesting(false)
            }
    }
}

/// The system blur behind glass. Materials are tuned by macOS, so the CSS 28px blur and
/// 160% saturation are approximations; the tint on top carries the contrast.
struct KzVisualEffect: NSViewRepresentable {
    let blending: KzGlassBlending
    let style: KzGlassStyle

    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        configure(view)
        return view
    }

    func updateNSView(_ view: NSVisualEffectView, context: Context) {
        configure(view)
    }

    private func configure(_ view: NSVisualEffectView) {
        switch blending {
        case .behindWindow:
            view.blendingMode = .behindWindow
            view.material = style == .strong ? .popover : .sidebar
            // Like a native sidebar, the blur goes flat when the window is inactive. Strong glass in its
            // own window (menus, popovers, panels) is never key, so it stays active like a native menu.
            view.state = style == .strong ? .active : .followsWindowActiveState
        case .withinWindow:
            view.blendingMode = .withinWindow
            view.material = style == .strong ? .popover : .headerView
            view.state = .active
        }
        view.isEmphasized = false
    }
}
