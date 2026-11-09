// shadcn's Button variants and sizes as a native `ButtonStyle` (`button.tsx`): use
// `Button { … }.buttonStyle(.kz(.outline, size: .sm))`. Variants keep the web's fills, depth,
// sheen, hover and pressed states; the press utility's scale; disabled at 50% opacity. Focus is the
// system focus ring.

import SwiftUI

/// Fill and interaction treatments for native buttons.
public enum ButtonVariant: String, CaseIterable, Sendable {
    case `default`, secondary, ghost, outline, destructive, link
}

/// Point sizes and spacing for Kudzu buttons.
public enum ButtonSize: String, CaseIterable, Sendable {
    case xs, sm, `default`, lg, icon, iconXs, iconSm, iconLg
    /// `size="lg" className="h-10 px-5"`: the first-run and permission call-to-action buttons.
    case xl

    /// `h-6 / h-7 / h-8 / h-9 / h-10`, `size-*` for icon sizes.
    public var height: CGFloat {
        switch self {
        case .xs, .iconXs: 24
        case .sm, .iconSm: 28
        case .default, .icon: 32
        case .lg, .iconLg: 36
        case .xl: 40
        }
    }

    /// Whether this size is a square icon button.
    public var isIcon: Bool { [.icon, .iconXs, .iconSm, .iconLg].contains(self) }

    /// `px-2` (xs) / `px-2.5` / `px-5` (xl).
    public var horizontalPadding: CGFloat {
        switch self {
        case _ where isIcon: 0
        case .xs: 8
        case .xl: 20
        default: 10
        }
    }
    /// Padding removed beside an inline icon (small buttons use `pl/pr-1.5`).
    public var inlineIconInset: CGFloat { self == .sm ? 4 : 2 }
    /// `gap-1` (xs, sm) / `gap-1.5`.
    public var gap: CGFloat { self == .xs || self == .sm ? 4 : 6 }
    /// `rounded-lg`, or `rounded-[min(var(--radius-md),10px|12px)]` for the small sizes.
    public var radius: CGFloat {
        switch self {
        case .xs, .iconXs: Radius.mdMax10
        case .sm, .iconSm: Radius.mdMax12
        default: Radius.lg
        }
    }
    /// `text-sm` (14), `text-[0.8rem]` (sm), `text-xs` (xs).
    public var fontSize: CGFloat {
        switch self {
        case .xs: 12
        case .sm: 12.8
        default: 14
        }
    }
    /// Icon box: `size-3` (xs), `size-3.5` (sm), `size-4`.
    public var iconSize: CGFloat {
        switch self {
        case .xs, .iconXs: 12
        case .sm: 14
        default: 16
        }
    }
}

/// Applies Kudzu’s fills, depth and feedback to a native button.
public struct KZButtonStyle: ButtonStyle {
    /// The visual treatment applied to the content.
    public var variant: ButtonVariant
    /// The point size or spacing variant.
    public var size: ButtonSize
    /// `aria-expanded` (a menu or popover this button opened is showing).
    public var expanded: Bool
    /// Anchored menu triggers stay still when pressed.
    public var menuTrigger: Bool = false
    /// `min-w-*`: the smallest width of the whole button, padding included (equal-width actions).
    public var minWidth: CGFloat?

    /// Creates KZButtonStyle with the supplied content and styling.
    public init(_ variant: ButtonVariant = .default, size: ButtonSize = .default, expanded: Bool = false, minWidth: CGFloat? = nil) {
        self.variant = variant
        self.size = size
        self.expanded = expanded
        self.minWidth = minWidth
    }

    /// Styles the native button’s label and interaction state.
    public func makeBody(configuration: Configuration) -> some View {
        KZButtonBody(configuration: configuration, variant: variant, size: size, expanded: expanded, menuTrigger: menuTrigger, minWidth: minWidth)
    }
}

extension ButtonStyle where Self == KZButtonStyle {
    /// `.buttonStyle(.kz(.ghost, size: .icon))`; `minWidth` is `min-w-*` (`.kz(.secondary, size: .sm, minWidth: 112)`).
    public static func kz(_ variant: ButtonVariant = .default, size: ButtonSize = .default, expanded: Bool = false, minWidth: CGFloat? = nil) -> KZButtonStyle {
        KZButtonStyle(variant, size: size, expanded: expanded, minWidth: minWidth)
    }
}

struct KZButtonBody: View {
    var configuration: ButtonStyleConfiguration
    var variant: ButtonVariant
    var size: ButtonSize
    var expanded: Bool
    var menuTrigger: Bool
    var minWidth: CGFloat?

    @State private var hovered = false
    @Environment(\.isEnabled) private var isEnabled
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let t = KZTheme(scheme)
        let pressed = configuration.isPressed
        let active = hovered || expanded
        let shape = RoundedRectangle(cornerRadius: size.radius, style: .circular)

        configuration.label
            .font(.system(size: size.fontSize, weight: .medium))
            .lineLimit(1)
            .iconSize(size.iconSize)
            .foregroundStyle(foreground(t, active: active))
            .underline(variant == .link && hovered, pattern: .solid)
            .padding(.horizontal, size.horizontalPadding)
            .frame(minWidth: size.isIcon ? size.height : minWidth, minHeight: size.height, maxHeight: size.height)
            .frame(width: size.isIcon ? size.height : nil)
            .background {
                if let fill = fill(t, active: active) {
                    Color.clear
                        .depth(depth(active: active, pressed: pressed), fill: fill, in: shape)
                    if hasSheen && !pressed {
                        shape.fill((variant == .default ? Sheen.strong : Sheen.normal).gradient)
                    }
                } else if variant == .ghost && pressed {
                    Color.clear.depth(.pressed, fill: t[.muted], in: shape)
                }
            }
            .contentShape(shape)
            .opacity(isEnabled ? 1 : 0.5)
            .onHover { hovered = $0 && isEnabled }
            .themeAwareAnimation(.kz.hover, value: active)
            .themeAwareAnimation(.kz.hover, value: pressed)
            .press(pressed, menuTrigger: menuTrigger)
            .environment(\.kzButtonSize, size)
            .environment(\.kzKbdContext, variant == .default ? .onFilledButton : .standard)
    }

    private var hasSheen: Bool { [.default, .outline, .secondary].contains(variant) }

    private func foreground(_ t: KZTheme, active: Bool) -> Color {
        switch variant {
        case .default: t[.primaryForeground]
        case .secondary: t[.secondaryForeground]
        case .outline, .ghost: t[.foreground]
        case .destructive: t[.destructive]
        case .link: t[.primary]
        }
    }

    private func fill(_ t: KZTheme, active: Bool) -> Color? {
        switch variant {
        case .default: active ? t[.primary, alpha: 0.9] : t[.primary]
        case .outline:
            active ? (t.dark && !expanded ? t.mixedWhite(.surfaceRaised, 0.04) : t[.muted]) : t[.surfaceRaised]
        case .secondary: active && !expanded ? t.mixed(.secondary, .foreground, 0.05) : t[.secondary]
        case .ghost: active ? (t.dark && !expanded ? t[.muted, alpha: 0.5] : t[.muted]) : nil
        case .destructive: t[.destructive, alpha: (t.dark ? 0.2 : 0.1) + (active ? 0.1 : 0)]
        case .link: nil
        }
    }

    private func depth(active: Bool, pressed: Bool) -> Depth {
        switch variant {
        case .default, .outline, .secondary: pressed ? .pressed : (active ? .raisedHover : .raised)
        case .ghost: pressed ? .pressed : .none
        case .destructive, .link: .none
        }
    }
}

extension KZButtonStyle {
    static func menuTrigger(_ variant: ButtonVariant, size: ButtonSize) -> Self {
        var style = Self(variant, size: size)
        style.menuTrigger = true
        return style
    }
}

// MARK: Label helper

/// A button label with an optional leading or trailing icon (`data-icon="inline-start|end"`, which
/// trims the padding on that side by 2 pt).
public struct ButtonLabel: View {
    /// The user-visible title.
    public var title: String
    /// Optional leading SF Symbol.
    public var icon: Image?
    /// Optional trailing SF Symbol.
    public var trailing: Image?

    /// Creates ButtonLabel with the supplied content and styling.
    public init(_ title: String, icon: Image? = nil, trailing: Image? = nil) {
        self.title = title
        self.icon = icon
        self.trailing = trailing
    }

    @Environment(\.kzButtonSize) private var size

    /// The composed native view.
    public var body: some View {
        HStack(spacing: size.gap) {
            if let icon { icon.icon().padding(.leading, -size.inlineIconInset) }
            Text(title)
            if let trailing { trailing.icon().padding(.trailing, -size.inlineIconInset) }
        }
    }
}

extension EnvironmentValues {
    @Entry var kzButtonSize: ButtonSize = .default
}
