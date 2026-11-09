// Background-image utilities: `sheen` / `sheen-strong` (top-lit gradients over the fill), `noise`
// (a 3 px dot grid), and the `animate-shimmer` loading sweep.

import AppKit
import SwiftUI

/// "Top-lit sheen for raised actions; layered over the background colour, so hover tints still work."
public enum Sheen: Hashable, Sendable {
    /// `linear-gradient(to bottom, oklch(1 0 0 / 0.1), oklch(1 0 0 / 0) 60%)`.
    case normal
    /// `linear-gradient(to bottom, oklch(1 0 0 / 0.2), oklch(1 0 0 / 0) 70%)`.
    case strong

    /// The `gradient` design-system value.
    public var gradient: LinearGradient {
        let (top, end): (Double, Double) = self == .normal ? (0.1, 0.6) : (0.2, 0.7)
        return LinearGradient(
            stops: [
                .init(color: Color.oklch(1, 0, 0, alpha: top), location: 0),
                .init(color: Color.oklch(1, 0, 0, alpha: 0), location: end),
            ],
            startPoint: .top, endPoint: .bottom)
    }
}

extension View {
    /// Overlays the sheen gradient in `shape` (use under content, over the fill).
    public func sheen<S: Shape>(_ sheen: Sheen = .normal, in shape: S) -> some View {
        background(shape.fill(sheen.gradient))
    }

    /// `.noise`: `radial-gradient(oklch(1 0 0 / 0.025) 1px, transparent 1px)` on a 3 px grid.
    public func noise() -> some View {
        background(Noise())
    }

    /// `animate-shimmer` over a `linear-gradient(90deg, transparent, color, transparent)` background
    /// sized 200% × 100%, in `shape`.
    public func shimmer<S: Shape>(_ color: Color, in shape: S) -> some View {
        background(Shimmer(color: color).clipShape(shape))
    }
}

/// The `.noise` dot texture: one 1 px-radius dot centred in every 3 × 3 tile.
public struct Noise: View {
    /// Creates Noise with the supplied content and styling.
    public init() {}

    private static let tile: NSImage = {
        let image = NSImage(size: NSSize(width: 3, height: 3), flipped: false) { _ in
            NSColor.oklch(OKLCH(1, 0, 0, alpha: 0.025)).setFill()
            NSBezierPath(ovalIn: NSRect(x: 0.5, y: 0.5, width: 2, height: 2)).fill()
            return true
        }
        return image
    }()

    /// The composed native view.
    public var body: some View {
        Image(nsImage: Self.tile)
            .resizable(resizingMode: .tile)
            .allowsHitTesting(false)
            .accessibilityHidden(true)
    }
}

/// `shimmer 1.6s linear infinite`: `background-position` from -200% to 200% with a 200%-wide,
/// repeating gradient tile, i.e. the highlight sweeps right to left twice per cycle. Driven by a
/// repeating linear animation of the tile offset. Under Reduce Motion the CSS runs it once in 1 ms
/// and leaves the background at its resting position (0%).
public struct Shimmer: View {
    /// The shadow’s OKLCH colour and opacity.
    public var color: Color
    @State private var running = false
    @Environment(\.accessibilityReduceMotion) private var reduce

    /// Creates Shimmer with the supplied content and styling.
    public init(color: Color) { self.color = color }

    /// The composed native view.
    public var body: some View {
        GeometryReader { proxy in
            let w = proxy.size.width
            // Offset of the tile's left edge: (W − 2W) × p, p from −2 (2W) to 2 (−2W).
            let offset = reduce ? 0 : (running ? -2 * w : 2 * w)
            HStack(spacing: 0) {
                ForEach(0..<4, id: \.self) { _ in
                    LinearGradient(colors: [color.opacity(0), color, color.opacity(0)], startPoint: .leading, endPoint: .trailing)
                        .frame(width: 2 * w)
                }
            }
            // Four tiles from (offset − 4W) cover [0, W] for every offset in [−2W, 2W].
            .offset(x: offset - 4 * w)
        }
        .clipped()
        .allowsHitTesting(false)
        .onAppear {
            guard !reduce else { return }
            withAnimation(.linear(duration: KeyframeSpec.shimmerPeriod).repeatForever(autoreverses: false)) { running = true }
        }
    }
}
