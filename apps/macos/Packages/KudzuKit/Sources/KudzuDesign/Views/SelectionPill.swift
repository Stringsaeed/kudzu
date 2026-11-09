// `SelectionPill` (`selection-pill.tsx`): one background for a group's selected option, sliding from
// option to option on the soft spring. Natively that is `matchedGeometryEffect`: every option asks for
// the pill with `.selectionPill(isSelected, in: namespace)`, and only the selected one draws it. With
// no selection the pill fades out; appearing, it fades in where it belongs instead of sliding there.
// The slide is exempt from theme-switch freezing (it only animates position and size).

import SwiftUI

/// How the pill looks: Segmented uses `rounded-md bg-surface-raised shadow-soft sheen`.
public struct SelectionPillStyle: Sendable {
    /// The corner or shadow radius in points.
    public var radius: CGFloat
    /// The surface’s colour token.
    public var fill: KZToken
    /// The surface’s native depth recipe.
    public var depth: Depth
    /// Whether the surface carries a top-lit sheen.
    public var sheen: Bool

    /// Creates SelectionPillStyle with the supplied content and styling.
    public init(radius: CGFloat = 6, fill: KZToken = .surfaceRaised, depth: Depth = .soft, sheen: Bool = true) {
        self.radius = radius
        self.fill = fill
        self.depth = depth
        self.sheen = sheen
    }

    /// The default segmented-selection surface.
    public static let segmented = SelectionPillStyle()
    /// Sidebar nav rows (`bg-foreground/[0.06]`-style selections use `.selected` depth).
    public static let nav = SelectionPillStyle(radius: Radius.md, fill: .surfaceRaised, depth: .selected, sheen: false)
}

extension View {
    /// Draws the group's sliding pill behind this option when `isSelected`.
    public func selectionPill(_ isSelected: Bool, in namespace: Namespace.ID, id: some Hashable = "selection-pill", style: SelectionPillStyle = .segmented) -> some View {
        modifier(SelectionPillModifier(isSelected: isSelected, namespace: namespace, id: AnyHashable(id), style: style))
    }
}

struct SelectionPillModifier: ViewModifier {
    var isSelected: Bool
    var namespace: Namespace.ID
    var id: AnyHashable
    var style: SelectionPillStyle
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduce

    func body(content: Content) -> some View {
        let shape = RoundedRectangle(cornerRadius: style.radius)
        content
            .background {
                if isSelected {
                    ZStack {
                        Color.clear.depth(style.depth, fill: KZTheme(scheme)[style.fill], in: shape)
                        if style.sheen { shape.fill(Sheen.normal.gradient) }
                    }
                    .matchedGeometryEffect(id: id, in: namespace)
                    .transition(.opacity.animation(.kz.honoring(reduce, .kz.ease(duration: Motion.fast))))
                }
            }
            .animation(.kz.honoring(reduce, .kz.springSoft(duration: Motion.slow)), value: isSelected)
    }
}
