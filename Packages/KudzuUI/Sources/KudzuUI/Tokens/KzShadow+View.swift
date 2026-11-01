import SwiftUI

extension View {
    /// Applies a Kudzu shadow token around `shape`, the way CSS `box-shadow` draws it.
    ///
    /// Drop layers are drawn only *outside* the shape (so they never show through translucent
    /// glass) with CSS blur and spread; inset layers (the glass top highlight) are drawn inside
    /// the shape's top edge, above the background. Pass `insetHighlights: false` when the view
    /// draws its own highlight, as `kzGlass` does.
    public func kzShadow<S: Shape>(_ shadow: KzShadow, in shape: S, insetHighlights: Bool = true) -> some View {
        modifier(KzShadowModifier(shadow: shadow, shape: shape, insetHighlights: insetHighlights))
    }
}

private struct KzShadowModifier<S: Shape>: ViewModifier {
    let shadow: KzShadow
    let shape: S
    let insetHighlights: Bool
    @Environment(\.colorScheme) private var colorScheme

    func body(content: Content) -> some View {
        let layers = shadow.layers(for: colorScheme)
        content
            .background {
                KzDropShadowView(layers: layers.filter { !$0.inset }, shape: shape)
                    .allowsHitTesting(false)
                    .accessibilityHidden(true)
            }
            .overlay {
                if insetHighlights {
                    KzInsetShadowView(layers: layers.filter(\.inset), shape: shape)
                        .allowsHitTesting(false)
                        .accessibilityHidden(true)
                }
            }
    }
}

/// Draws drop-shadow layers outside a shape (CSS semantics: nothing beneath the shape itself).
struct KzDropShadowView<S: Shape>: View {
    let layers: [KzShadowLayer]
    let shape: S

    var body: some View {
        // The canvas must be larger than the shape so the blur isn't clipped.
        let pad = layers.map { $0.blur * 1.5 + max(abs($0.x), abs($0.y)) + max(0, $0.spread) }.max() ?? 0
        Canvas { context, size in
            let rect = CGRect(origin: .zero, size: size).insetBy(dx: pad, dy: pad)
            let shapePath = shape.path(in: rect)
            // Clip away the shape's own area.
            var outside = Path(CGRect(origin: .zero, size: size))
            outside.addPath(shapePath)
            context.clip(to: outside, style: FillStyle(eoFill: true))
            for layer in layers.reversed() {
                var ctx = context
                let spread = layer.spread
                let shadowRect = rect.insetBy(dx: -spread, dy: -spread)
                // CSS blur radius is 2σ; SwiftUI's shadow radius is σ-like (half the CSS value).
                ctx.addFilter(.shadow(color: layer.color.color, radius: layer.blur / 2, x: layer.x, y: layer.y, options: .shadowOnly))
                ctx.fill(shape.path(in: shadowRect.width > 0 && shadowRect.height > 0 ? shadowRect : rect), with: .color(.black))
            }
        }
        .padding(-pad)
    }
}

/// Draws blur-free inset layers: the band between the shape and the shape offset by (x, y).
struct KzInsetShadowView<S: Shape>: View {
    let layers: [KzShadowLayer]
    let shape: S

    var body: some View {
        Canvas { context, size in
            let rect = CGRect(origin: .zero, size: size)
            let shapePath = shape.path(in: rect)
            context.clip(to: shapePath)
            for layer in layers {
                var band = shapePath
                band.addPath(shapePath.offsetBy(dx: layer.x, dy: layer.y))
                context.fill(band, with: .color(layer.color.color), style: FillStyle(eoFill: true))
            }
        }
    }
}
