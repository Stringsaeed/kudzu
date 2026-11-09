// `.vibrant`: on macOS the webview root gets the class when native vibrancy is behind it, and the
// `vibrant:` variant and the `.vibrant { --sidebar: … }` override switch to translucent fills so the
// `NSVisualEffectView` shows through. Here a subtree opts in with `VibrantSurface` (or
// `.vibrant()`), and token-aware styles read `\.kzVibrant`.

import SwiftUI

extension EnvironmentValues {
    /// Whether this subtree sits over native vibrancy (`html.vibrant`).
    @Entry public var kzVibrant: Bool = false
}

extension View {
    /// Applies the `.vibrant` token overrides to this subtree.
    public func vibrant(_ on: Bool = true) -> some View {
        environment(\.kzVibrant, on)
    }

    /// `vibrant:` variant helper: one value normally, another over vibrancy.
    public func ifVibrant<V: View>(@ViewBuilder _ transform: @escaping (Self) -> V) -> some View {
        VibrantSwitch(base: self, transform: transform)
    }
}

private struct VibrantSwitch<Base: View, V: View>: View {
    var base: Base
    var transform: (Base) -> V
    @Environment(\.kzVibrant) private var vibrant
    var body: some View {
        if vibrant { transform(base) } else { base }
    }
}

/// A container whose subtree uses the vibrant token overrides, optionally drawing the material.
public struct VibrantSurface<Content: View>: View {
    var effect: VisualEffect?
    var content: Content

    /// Creates VibrantSurface with the supplied content and styling.
    public init(_ effect: VisualEffect? = nil, @ViewBuilder content: () -> Content) {
        self.effect = effect
        self.content = content()
    }

    /// The composed native view.
    public var body: some View {
        content
            .vibrant()
            .background {
                if let effect { effect.ignoresSafeArea() }
            }
    }
}

/// Token fills that change over vibrancy. `--sidebar` is the only token `.vibrant` overrides.
public struct SidebarFill: ShapeStyle {
    /// Creates SidebarFill with the supplied content and styling.
    public init() {}
    /// Resolves the token using the view’s appearance and vibrancy.
    public func resolve(in environment: EnvironmentValues) -> Color.Resolved {
        let token: KZToken = environment.kzVibrant ? .sidebarVibrant : .sidebar
        return Color.oklch(Palette.value(token, dark: environment.colorScheme == .dark)).resolve(in: environment)
    }
}

/// Any token resolved by the environment's colour scheme (not the window appearance), so a subtree
/// can preview the other theme with `.environment(\.colorScheme, …)`.
public struct TokenFill: ShapeStyle {
    /// The `token` design-system value.
    public var token: KZToken
    /// Opacity from zero to one.
    public var alpha: Double

    /// Creates TokenFill with the supplied content and styling.
    public init(_ token: KZToken, alpha: Double = 1) {
        self.token = token
        self.alpha = alpha
    }

    /// Resolves the token using the view’s appearance and vibrancy.
    public func resolve(in environment: EnvironmentValues) -> Color.Resolved {
        let value = Palette.value(token, dark: environment.colorScheme == .dark).scaledAlpha(alpha)
        return Color.oklch(value).resolve(in: environment)
    }
}

extension ShapeStyle where Self == SidebarFill {
    /// `bg-sidebar`, translucent over vibrancy.
    public static var kzSidebar: SidebarFill { SidebarFill() }
}

extension ShapeStyle where Self == TokenFill {
    /// A token as a scheme-resolved style: `.foregroundStyle(.token(.mutedForeground))`.
    public static func token(_ token: KZToken, alpha: Double = 1) -> TokenFill { TokenFill(token, alpha: alpha) }
}
