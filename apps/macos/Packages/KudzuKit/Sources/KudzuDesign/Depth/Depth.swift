// Depth: the intent of the `--elev-*` tokens with native pieces. "Surfaces sit on the page (card),
// actions sit on surfaces (raised), fields sink into them (well); overlays float above everything."
// "Depth comes from 1px hairlines + very soft shadows, never from heavy borders."
//
// Each level is at most two native drop shadows, native inner shadows for sunken fields,
// clipped edge strips for top lights and keycap lips, and a hairline stroke. Light
// surfaces need far less shadow than dark ones; dark ones get a faint top edge and a dark contact
// shadow because the light comes from above. Values follow the CSS intent; they are not a port of
// the layer lists. Tailwind's stock names map like `@theme inline` does (`shadow-xs` → raised, …).

import SwiftUI

/// A native surface recipe for each appearance.
public struct Depth: Hashable, Sendable {
    /// A drop shadow: SwiftUI radius (≈ half the CSS blur), y offset, colour.
    public struct Drop: Hashable, Sendable {
        /// The corner or shadow radius in points.
        public var radius: CGFloat
        /// Vertical shadow offset in points.
        public var y: CGFloat
        /// The shadow’s OKLCH colour and opacity.
        public var color: OKLCH
        /// Creates Drop with the supplied content and styling.
        public init(radius: CGFloat, y: CGFloat, color: OKLCH) {
            self.radius = radius
            self.y = y
            self.color = color
        }
    }

    /// Inset shading (y > 0 shades from the top). Radius 0 draws a clipped top/bottom edge strip.
    public struct Inner: Hashable, Sendable {
        /// The corner or shadow radius in points.
        public var radius: CGFloat
        /// Vertical shadow offset in points.
        public var y: CGFloat
        /// The shadow’s OKLCH colour and opacity.
        public var color: OKLCH
        /// Creates Inner with the supplied content and styling.
        public init(radius: CGFloat, y: CGFloat, color: OKLCH) {
            self.radius = radius
            self.y = y
            self.color = color
        }
    }

    /// One appearance's recipe.
    public struct Recipe: Hashable, Sendable {
        /// At most two native outer shadows.
        public var drops: [Drop]
        /// Native inset shading and top or bottom edge highlights.
        public var inner: [Inner]
        /// Optional surface edge colour.
        public var hairline: OKLCH?
        /// Surface edge width in points.
        public var hairlineWidth: CGFloat
        /// Creates Recipe with the supplied content and styling.
        public init(drops: [Drop] = [], inner: [Inner] = [], hairline: OKLCH? = nil, hairlineWidth: CGFloat = 1) {
            self.drops = drops
            self.inner = inner
            self.hairline = hairline
            self.hairlineWidth = hairlineWidth
        }
    }

    /// The design token’s stable name.
    public var name: String
    /// Surface recipe for light appearance.
    public var light: Recipe
    /// Surface recipe for dark appearance.
    public var dark: Recipe

    /// Returns the surface recipe for the requested appearance.
    public func recipe(dark isDark: Bool) -> Recipe { isDark ? dark : light }

    // MARK: Colours

    /// Light theme shadow ink (`oklch(0.147 0.01 50 / a)`).
    static func ink(_ a: Double) -> OKLCH { OKLCH(0.147, 0.01, 50, alpha: a) }
    /// Light theme hairline ink (`oklch(0.147 0.004 49.25 / a)`).
    static func line(_ a: Double) -> OKLCH { OKLCH(0.147, 0.004, 49.25, alpha: a) }
    static func white(_ a: Double) -> OKLCH { OKLCH(1, 0, 0, alpha: a) }
    static func black(_ a: Double) -> OKLCH { OKLCH(0, 0, 0, alpha: a) }

    // MARK: Levels

    /// The `none` design-system value.
    public static let none = Depth(name: "none", light: Recipe(), dark: Recipe())

    /// `shadow-hairline`: a 1 px `--hairline` edge.
    public static let hairline = Depth(
        name: "hairline",
        light: Recipe(hairline: Palette.value(.hairline, dark: false)),
        dark: Recipe(hairline: Palette.value(.hairline, dark: true)))

    /// Content cards on the page (`shadow-card`).
    public static let card = Depth(
        name: "card",
        light: Recipe(drops: [Drop(radius: 1, y: 1, color: ink(0.05)), Drop(radius: 6, y: 4, color: ink(0.06))], hairline: line(0.07)),
        dark: Recipe(drops: [Drop(radius: 1, y: 1, color: black(0.25)), Drop(radius: 10, y: 6, color: black(0.3))],
                     inner: [Inner(radius: 0, y: 1, color: white(0.045))], hairline: white(0.06)))

    /// Buttons and selects on a surface (`shadow-raised`, Tailwind `shadow-xs`).
    public static let raised = Depth(
        name: "raised",
        light: Recipe(drops: [Drop(radius: 1, y: 1, color: ink(0.08))], inner: [Inner(radius: 0, y: 1, color: white(0.7))], hairline: line(0.1)),
        dark: Recipe(drops: [Drop(radius: 1, y: 1, color: black(0.35)), Drop(radius: 2.5, y: 2, color: black(0.2))],
                     inner: [Inner(radius: 0, y: 1, color: white(0.08))], hairline: white(0.07)))

    /// Raised, hovered (`shadow-raised-hover`).
    public static let raisedHover = Depth(
        name: "raised-hover",
        light: Recipe(drops: [Drop(radius: 2.5, y: 2, color: ink(0.12))], inner: [Inner(radius: 0, y: 1, color: white(0.7))], hairline: line(0.13)),
        dark: Recipe(drops: [Drop(radius: 1, y: 1, color: black(0.4)), Drop(radius: 4, y: 4, color: black(0.3))],
                     inner: [Inner(radius: 0, y: 1, color: white(0.1))], hairline: white(0.1)))

    /// Raised, held down (`shadow-pressed`): shading from the top, no lift.
    public static let pressed = Depth(
        name: "pressed",
        light: Recipe(inner: [Inner(radius: 1, y: 1, color: ink(0.12))], hairline: line(0.1)),
        dark: Recipe(inner: [Inner(radius: 1.5, y: 1, color: black(0.35))], hairline: white(0.06)))

    /// Fields sunk into a surface (`shadow-well`).
    public static let well = Depth(
        name: "well",
        light: Recipe(inner: [Inner(radius: 1, y: 1, color: ink(0.06))], hairline: line(0.09)),
        dark: Recipe(drops: [Drop(radius: 0, y: 1, color: white(0.03))], inner: [Inner(radius: 1, y: 1, color: black(0.3))], hairline: white(0.07)))

    /// Switch knobs and slider thumbs (`shadow-knob`).
    public static let knob = Depth(
        name: "knob",
        light: Recipe(drops: [Drop(radius: 1.5, y: 1, color: ink(0.2))], hairline: ink(0.12), hairlineWidth: 0.5),
        dark: Recipe(drops: [Drop(radius: 1.5, y: 1, color: black(0.45))], hairline: black(0.3), hairlineWidth: 0.5))

    /// Keycaps (`shadow-key`): a darker bottom edge.
    public static let key = Depth(
        name: "key",
        light: Recipe(inner: [Inner(radius: 0, y: -1, color: line(0.16))], hairline: line(0.08), hairlineWidth: 0.5),
        dark: Recipe(inner: [Inner(radius: 0, y: 1, color: white(0.06)), Inner(radius: 0, y: -1, color: black(0.45))], hairline: black(0.3), hairlineWidth: 0.5))

    /// The selected row or option (`shadow-selected`).
    public static let selected = Depth(
        name: "selected",
        light: Recipe(drops: [Drop(radius: 1, y: 1, color: ink(0.06))], hairline: line(0.05)),
        dark: Recipe(drops: [Drop(radius: 1, y: 1, color: black(0.2))], inner: [Inner(radius: 0, y: 1, color: white(0.05))]))

    /// Iris-filled actions (`shadow-accent`): a blue edge and a soft blue glow.
    public static let accent = Depth(
        name: "accent",
        light: Recipe(drops: [Drop(radius: 1, y: 1, color: OKLCH(0.424, 0.199, 265.638, alpha: 0.25)), Drop(radius: 5, y: 4, color: OKLCH(0.488, 0.243, 264.376, alpha: 0.3))],
                      inner: [Inner(radius: 0, y: 1, color: white(0.22))], hairline: OKLCH(0.424, 0.199, 265.638, alpha: 0.6)),
        dark: Recipe(drops: [Drop(radius: 1, y: 1, color: black(0.4)), Drop(radius: 6, y: 4, color: OKLCH(0.623, 0.214, 259.815, alpha: 0.3))],
                     inner: [Inner(radius: 0, y: 1, color: white(0.35))], hairline: OKLCH(0.623, 0.214, 259.815, alpha: 0.5)))

    /// Toggled-on toggles and segmented thumbs (`shadow-soft`, Tailwind `shadow-sm`).
    public static let soft = Depth(
        name: "soft",
        light: Recipe(drops: [Drop(radius: 1, y: 1, color: ink(0.07)), Drop(radius: 3, y: 2, color: ink(0.06))], inner: [Inner(radius: 0, y: 1, color: white(0.7))], hairline: line(0.06)),
        dark: Recipe(drops: [Drop(radius: 1, y: 1, color: black(0.3)), Drop(radius: 5, y: 4, color: black(0.2))], inner: [Inner(radius: 0, y: 1, color: white(0.07))], hairline: white(0.05)))

    /// Menus, popovers, tooltips (`shadow-float`, Tailwind `shadow-md/lg`).
    public static let float = Depth(
        name: "float",
        light: Recipe(drops: [Drop(radius: 3, y: 2, color: ink(0.06)), Drop(radius: 14, y: 12, color: ink(0.12))], hairline: Palette.value(.hairline, dark: false)),
        dark: Recipe(drops: [Drop(radius: 3, y: 2, color: black(0.2)), Drop(radius: 14, y: 12, color: black(0.35))], hairline: Palette.value(.hairline, dark: true)))

    /// Dialogs and floating panels (`shadow-panel`, Tailwind `shadow-xl/2xl`).
    public static let panel = Depth(
        name: "panel",
        light: Recipe(drops: [Drop(radius: 7, y: 4, color: ink(0.08)), Drop(radius: 28, y: 24, color: ink(0.2))], hairline: Palette.value(.hairline, dark: false)),
        dark: Recipe(drops: [Drop(radius: 8, y: 4, color: black(0.25)), Drop(radius: 30, y: 24, color: black(0.45))], hairline: Palette.value(.hairline, dark: true)))

    /// Every level, for the gallery and tests.
    public static let all: [Depth] = [.hairline, .card, .raised, .raisedHover, .pressed, .well, .knob, .key, .selected, .accent, .soft, .float, .panel]

    // Tailwind's stock scale, remapped as in `@theme inline`.
    /// Tailwind’s `shadowXs` alias on Kudzu’s depth scale.
    public static let shadowXs = raised
    /// Tailwind’s `shadowSm` alias on Kudzu’s depth scale.
    public static let shadowSm = soft
    /// Tailwind’s `shadowMd` alias on Kudzu’s depth scale.
    public static let shadowMd = float
    /// Tailwind’s `shadowLg` alias on Kudzu’s depth scale.
    public static let shadowLg = float
    /// Tailwind’s `shadowXl` alias on Kudzu’s depth scale.
    public static let shadowXl = panel
}

// MARK: Modifier

extension View {
    /// Draws a surface behind this view: `fill` in `shape` with the depth's inner shadows, drop
    /// shadows outside, and its hairline.
    public func depth<S: InsettableShape, F: ShapeStyle>(_ depth: Depth, fill: F, in shape: S) -> some View {
        modifier(DepthModifier(depth: depth, fill: AnyShapeStyle(fill), shape: shape))
    }

    /// Adds a hairline and native drop shadows to existing content; inner shading requires a fill.
    public func depth<S: InsettableShape>(_ depth: Depth, in shape: S) -> some View {
        modifier(DepthModifier(depth: depth, fill: nil, shape: shape))
    }
}

struct DepthModifier<S: InsettableShape>: ViewModifier {
    var depth: Depth
    var fill: AnyShapeStyle?
    var shape: S
    @Environment(\.colorScheme) private var scheme

    func body(content: Content) -> some View {
        let r = depth.recipe(dark: scheme == .dark)
        Group {
            if fill != nil {
                content.background { surface(r).modifier(DropShadows(drops: r.drops)) }
            } else {
                content.modifier(DropShadows(drops: r.drops))
            }
        }
            .overlay {
                if let line = r.hairline {
                    shape.strokeBorder(Color.oklch(line), lineWidth: r.hairlineWidth).allowsHitTesting(false)
                }
            }
    }

    private func surface(_ r: Depth.Recipe) -> some View {
        // Zero-radius white inner shadows wash the whole fill on current SwiftUI renderers.
        // A clipped native edge keeps the intended one-point top light / bottom keycap lip.
        let shadows = r.inner.filter { $0.radius > 0 }
        return shadedFill(shadows)
            .overlay(alignment: .top) {
                if let edge = r.inner.first(where: { $0.radius == 0 && $0.y > 0 }) {
                    Rectangle().fill(Color.oklch(edge.color)).frame(height: edge.y)
                }
            }
            .overlay(alignment: .bottom) {
                if let edge = r.inner.first(where: { $0.radius == 0 && $0.y < 0 }) {
                    Rectangle().fill(Color.oklch(edge.color)).frame(height: -edge.y)
                }
            }
            .clipShape(shape)
    }

    @ViewBuilder
    private func shadedFill(_ shadows: [Depth.Inner]) -> some View {
        let base = fill ?? AnyShapeStyle(Color.clear)
        switch shadows.count {
        case 0: shape.fill(base)
        case 1: shape.fill(base.shadow(Self.inner(shadows[0])))
        default: shape.fill(base.shadow(Self.inner(shadows[0])).shadow(Self.inner(shadows[1])))
        }
    }

    static func inner(_ i: Depth.Inner) -> ShadowStyle {
        .inner(color: Color.oklch(i.color), radius: i.radius, x: 0, y: i.y)
    }
}

private struct DropShadows: ViewModifier {
    var drops: [Depth.Drop]
    func body(content: Content) -> some View {
        content
            .shadow(color: drops.count > 0 ? Color.oklch(drops[0].color) : .clear, radius: drops.first?.radius ?? 0, y: drops.first?.y ?? 0)
            .shadow(color: drops.count > 1 ? Color.oklch(drops[1].color) : .clear, radius: drops.count > 1 ? drops[1].radius : 0, y: drops.count > 1 ? drops[1].y : 0)
    }
}
