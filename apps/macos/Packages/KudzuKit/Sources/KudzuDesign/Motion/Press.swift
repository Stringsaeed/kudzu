// "Press feedback for anything clickable: quick to sink, springy on release. Also carries the hover
// transition, so use it instead of transition-colors/all. Menu triggers stay still, because the
// menu is anchored to them. Tune the depth per element with [--press-scale:…]."
//
// CSS: `scale` transitions over `--motion-slow` with `--spring-pop`; while `:active` it switches to
// `--motion-fast` `ease-out` and scales to `--press-scale` (0.97). Disabled elements and menu
// triggers (`[aria-haspopup]`) don't scale.

import SwiftUI

/// Press-scale values for actions and navigation rows.
public enum Press {
    /// `--press-scale` default.
    public static let scale: CGFloat = 0.97
    /// The nav rows' tighter press (`[--press-scale:0.985]`).
    public static let subtle: CGFloat = 0.985
}

struct PressModifier: ViewModifier {
    var isPressed: Bool
    var scale: CGFloat
    var isMenuTrigger: Bool
    @Environment(\.isEnabled) private var isEnabled
    @Environment(\.accessibilityReduceMotion) private var reduce

    func body(content: Content) -> some View {
        let active = isPressed && isEnabled && !isMenuTrigger
        content
            .scaleEffect(active ? scale : 1)
            .animation(.kz.honoring(reduce, active ? .kz.easeOut(duration: Motion.fast) : .kz.springPop(duration: Motion.slow)), value: active)
    }
}

extension View {
    /// The `press` utility's scale feedback, driven by a pressed state (a `ButtonStyle`'s
    /// `configuration.isPressed`). Colour/shadow changes should animate with `Animation.kz.hover`.
    public func press(_ isPressed: Bool, scale: CGFloat = Press.scale, menuTrigger: Bool = false) -> some View {
        modifier(PressModifier(isPressed: isPressed, scale: scale, isMenuTrigger: menuTrigger))
    }
}

/// A chrome-less button style with `press` feedback and the hover transition, for custom clickable
/// rows and tiles (`className="… press …"` on a plain `<button>`).
public struct PressButtonStyle: ButtonStyle {
    /// The `scale` design-system value.
    public var scale: CGFloat
    /// The `menuTrigger` design-system value.
    public var menuTrigger: Bool

    /// Creates PressButtonStyle with the supplied content and styling.
    public init(scale: CGFloat = Press.scale, menuTrigger: Bool = false) {
        self.scale = scale
        self.menuTrigger = menuTrigger
    }

    /// Styles the native button’s label and interaction state.
    public func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .contentShape(Rectangle())
            .press(configuration.isPressed, scale: scale, menuTrigger: menuTrigger)
    }
}

extension ButtonStyle where Self == PressButtonStyle {
    /// `.buttonStyle(.press)`.
    public static var press: PressButtonStyle { PressButtonStyle() }
    /// `.buttonStyle(.press(scale: Press.subtle))`.
    public static func press(scale: CGFloat, menuTrigger: Bool = false) -> PressButtonStyle {
        PressButtonStyle(scale: scale, menuTrigger: menuTrigger)
    }
}

// MARK: Hover

/// Tracks hover for styling, the SwiftUI side of `:hover` (pointer only; no animation itself).
struct HoverTracker: ViewModifier {
    @Binding var hovered: Bool
    func body(content: Content) -> some View {
        content.onHover { hovered = $0 }
    }
}
