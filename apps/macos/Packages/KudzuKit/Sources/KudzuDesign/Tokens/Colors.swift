// Tokens as dynamic colours: `NSColor.kz.<token>` and `Color.kz.<token>` resolve light or dark by
// the effective `NSAppearance` (the app sets `NSApp.appearance` from `settings.theme`; `system`
// leaves it nil), exactly like `.dark` on the webview's root element.

import AppKit
import SwiftUI

// MARK: Appearance

/// Resolves AppKit appearances to Kudzu’s light or dark palette.
public enum KZAppearance {
    /// Whether an appearance uses the `.dark` token set.
    public static func isDark(_ appearance: NSAppearance) -> Bool {
        let match = appearance.bestMatch(from: [
            .aqua, .darkAqua, .vibrantLight, .vibrantDark,
            .accessibilityHighContrastAqua, .accessibilityHighContrastDarkAqua,
            .accessibilityHighContrastVibrantLight, .accessibilityHighContrastVibrantDark,
        ])
        switch match {
        case .darkAqua?, .vibrantDark?, .accessibilityHighContrastDarkAqua?, .accessibilityHighContrastVibrantDark?: return true
        default: return false
        }
    }
}

extension NSColor {
    /// A non-dynamic sRGB colour from an OKLCH value.
    public static func oklch(_ value: OKLCH) -> NSColor {
        let c = value.srgb
        return NSColor(srgbRed: c.r, green: c.g, blue: c.b, alpha: c.a)
    }

    /// A colour that resolves to `light` or `dark` by the drawing appearance.
    public static func dynamic(light: OKLCH, dark: OKLCH, name: String? = nil) -> NSColor {
        let l = NSColor.oklch(light).srgbComponents, d = NSColor.oklch(dark).srgbComponents
        return NSColor(name: name.map { NSColor.Name("kz." + $0) }) { appearance in
            let v = KZAppearance.isDark(appearance) ? d : l
            return NSColor(srgbRed: v.0, green: v.1, blue: v.2, alpha: v.3)
        }
    }

    fileprivate var srgbComponents: (CGFloat, CGFloat, CGFloat, CGFloat) {
        let c = usingColorSpace(.sRGB) ?? self
        return (c.redComponent, c.greenComponent, c.blueComponent, c.alphaComponent)
    }

    /// Kudzu tokens: `NSColor.kz.background`, `NSColor.kz.irisSoft`, …
    public static let kz = KZNSColors()
}

extension Color {
    /// A non-dynamic sRGB colour from an OKLCH value.
    public static func oklch(_ l: Double, _ c: Double, _ h: Double, alpha: Double = 1) -> Color {
        oklch(OKLCH(l, c, h, alpha: alpha))
    }

    /// Applies the oklch design-system operation.
    public static func oklch(_ value: OKLCH) -> Color {
        let c = value.srgb
        return Color(.sRGB, red: c.r, green: c.g, blue: c.b, opacity: c.a)
    }
}

extension ShapeStyle where Self == Color {
    /// Kudzu tokens: `Color.kz.background`, or `.foregroundStyle(.kz.mutedForeground)`.
    public static var kz: KZColors { KZColors() }
}

// MARK: NSColor set

/// Dynamic `NSColor`s for every token.
public struct KZNSColors: Sendable {
    /// Resolves the requested colour token.
    public subscript(_ token: KZToken) -> NSColor {
        NSColor.dynamic(light: Palette.value(token, dark: false), dark: Palette.value(token, dark: true), name: token.rawValue)
    }

    /// `color-mix(in oklch, a (1 - amount), b amount)` per theme.
    public func mixed(_ a: KZToken, with b: KZToken, amount: Double) -> NSColor {
        NSColor.dynamic(
            light: Palette.value(a, dark: false).mixed(with: Palette.value(b, dark: false), amount: amount),
            dark: Palette.value(a, dark: true).mixed(with: Palette.value(b, dark: true), amount: amount))
    }

    /// `color-mix(in oklch, token (1 - amount), fixed amount)`, e.g. `white 4%`.
    public func mixed(_ a: KZToken, with fixed: OKLCH, amount: Double) -> NSColor {
        NSColor.dynamic(
            light: Palette.value(a, dark: false).mixed(with: fixed, amount: amount),
            dark: Palette.value(a, dark: true).mixed(with: fixed, amount: amount))
    }

    /// The token with its alpha multiplied (`bg-primary/90`, `color-mix(in oklch, X p%, transparent)`).
    public func mix(_ token: KZToken, alpha: Double) -> NSColor {
        NSColor.dynamic(
            light: Palette.value(token, dark: false).scaledAlpha(alpha),
            dark: Palette.value(token, dark: true).scaledAlpha(alpha))
    }

    /// The dynamic `--background` colour token.
    public var background: NSColor { self[.background] }
    /// The dynamic `--foreground` colour token.
    public var foreground: NSColor { self[.foreground] }
    /// The dynamic `--card` colour token.
    public var card: NSColor { self[.card] }
    /// The dynamic `--card-foreground` colour token.
    public var cardForeground: NSColor { self[.cardForeground] }
    /// The dynamic `--popover` colour token.
    public var popover: NSColor { self[.popover] }
    /// The dynamic `--popover-foreground` colour token.
    public var popoverForeground: NSColor { self[.popoverForeground] }
    /// The dynamic `--primary` colour token.
    public var primary: NSColor { self[.primary] }
    /// The dynamic `--primary-foreground` colour token.
    public var primaryForeground: NSColor { self[.primaryForeground] }
    /// The dynamic `--secondary` colour token.
    public var secondary: NSColor { self[.secondary] }
    /// The dynamic `--secondary-foreground` colour token.
    public var secondaryForeground: NSColor { self[.secondaryForeground] }
    /// The dynamic `--muted` colour token.
    public var muted: NSColor { self[.muted] }
    /// The dynamic `--muted-foreground` colour token.
    public var mutedForeground: NSColor { self[.mutedForeground] }
    /// The dynamic `--accent` colour token.
    public var accent: NSColor { self[.accent] }
    /// The dynamic `--accent-foreground` colour token.
    public var accentForeground: NSColor { self[.accentForeground] }
    /// The dynamic `--destructive` colour token.
    public var destructive: NSColor { self[.destructive] }
    /// The dynamic `--border` colour token.
    public var border: NSColor { self[.border] }
    /// The dynamic `--input` colour token.
    public var input: NSColor { self[.input] }
    /// The dynamic `--ring` colour token.
    public var ring: NSColor { self[.ring] }
    /// The dynamic `--chart1` colour token.
    public var chart1: NSColor { self[.chart1] }
    /// The dynamic `--chart2` colour token.
    public var chart2: NSColor { self[.chart2] }
    /// The dynamic `--chart3` colour token.
    public var chart3: NSColor { self[.chart3] }
    /// The dynamic `--chart4` colour token.
    public var chart4: NSColor { self[.chart4] }
    /// The dynamic `--chart5` colour token.
    public var chart5: NSColor { self[.chart5] }
    /// The dynamic `--sidebar` colour token.
    public var sidebar: NSColor { self[.sidebar] }
    /// The dynamic `--sidebar-foreground` colour token.
    public var sidebarForeground: NSColor { self[.sidebarForeground] }
    /// The dynamic `--sidebar-primary` colour token.
    public var sidebarPrimary: NSColor { self[.sidebarPrimary] }
    /// The dynamic `--sidebar-primary-foreground` colour token.
    public var sidebarPrimaryForeground: NSColor { self[.sidebarPrimaryForeground] }
    /// The dynamic `--sidebar-accent` colour token.
    public var sidebarAccent: NSColor { self[.sidebarAccent] }
    /// The dynamic `--sidebar-accent-foreground` colour token.
    public var sidebarAccentForeground: NSColor { self[.sidebarAccentForeground] }
    /// The dynamic `--sidebar-border` colour token.
    public var sidebarBorder: NSColor { self[.sidebarBorder] }
    /// The dynamic `--sidebar-ring` colour token.
    public var sidebarRing: NSColor { self[.sidebarRing] }
    /// The dynamic `--surface` colour token.
    public var surface: NSColor { self[.surface] }
    /// The dynamic `--surface-raised` colour token.
    public var surfaceRaised: NSColor { self[.surfaceRaised] }
    /// The dynamic `--hairline` colour token.
    public var hairline: NSColor { self[.hairline] }
    /// The dynamic `--faint` colour token.
    public var faint: NSColor { self[.faint] }
    /// The dynamic `--iris` colour token.
    public var iris: NSColor { self[.iris] }
    /// The dynamic `--iris-soft` colour token.
    public var irisSoft: NSColor { self[.irisSoft] }
    /// The dynamic `--iris-foreground` colour token.
    public var irisForeground: NSColor { self[.irisForeground] }
    /// The dynamic `--iris-contrast` colour token.
    public var irisContrast: NSColor { self[.irisContrast] }
    /// The dynamic `--chip-time` colour token.
    public var chipTime: NSColor { self[.chipTime] }
    /// The dynamic `--chip-clipboard` colour token.
    public var chipClipboard: NSColor { self[.chipClipboard] }
    /// The dynamic `--chip-cursor` colour token.
    public var chipCursor: NSColor { self[.chipCursor] }
    /// The dynamic `--chip-form` colour token.
    public var chipForm: NSColor { self[.chipForm] }
    /// The dynamic `--chip-error` colour token.
    public var chipError: NSColor { self[.chipError] }
    /// The dynamic `--chip-neutral` colour token.
    public var chipNeutral: NSColor { self[.chipNeutral] }
    /// The dynamic `--success` colour token.
    public var success: NSColor { self[.success] }
    /// The dynamic `--warning` colour token.
    public var warning: NSColor { self[.warning] }
    /// The dynamic `--favorite` colour token.
    public var favorite: NSColor { self[.favorite] }
    /// The dynamic `--sidebar-vibrant` colour token.
    public var sidebarVibrant: NSColor { self[.sidebarVibrant] }
    /// The dynamic `--scrollbar-thumb` colour token.
    public var scrollbarThumb: NSColor { self[.scrollbarThumb] }
}

// MARK: SwiftUI set

/// Dynamic SwiftUI `Color`s for every token (backed by the dynamic `NSColor`s).
public struct KZColors: Sendable {
    /// Resolves the requested colour token.
    public subscript(_ token: KZToken) -> Color { Color(nsColor: NSColor.kz[token]) }

    /// `color-mix(in oklch, X p%, transparent)` = X at alpha p. Works on any colour.
    public func mix(_ color: Color, alpha: Double) -> Color { color.opacity(alpha) }
    /// A token at a multiplied alpha (`bg-foreground/[0.025]`, `bg-iris/80`).
    public func mix(_ token: KZToken, alpha: Double) -> Color { Color(nsColor: NSColor.kz.mix(token, alpha: alpha)) }
    /// `color-mix(in oklch, a, b amount)`, e.g. `color-mix(in oklch, var(--secondary), var(--foreground) 5%)`.
    public func mixed(_ a: KZToken, with b: KZToken, amount: Double) -> Color { Color(nsColor: NSColor.kz.mixed(a, with: b, amount: amount)) }
    /// `color-mix(in oklch, token, fixed amount)`, e.g. `color-mix(in oklch, var(--surface-raised), white 4%)`.
    public func mixed(_ a: KZToken, with fixed: OKLCH, amount: Double) -> Color { Color(nsColor: NSColor.kz.mixed(a, with: fixed, amount: amount)) }

    /// The dynamic `--background` colour token.
    public var background: Color { self[.background] }
    /// The dynamic `--foreground` colour token.
    public var foreground: Color { self[.foreground] }
    /// The dynamic `--card` colour token.
    public var card: Color { self[.card] }
    /// The dynamic `--card-foreground` colour token.
    public var cardForeground: Color { self[.cardForeground] }
    /// The dynamic `--popover` colour token.
    public var popover: Color { self[.popover] }
    /// The dynamic `--popover-foreground` colour token.
    public var popoverForeground: Color { self[.popoverForeground] }
    /// The dynamic `--primary` colour token.
    public var primary: Color { self[.primary] }
    /// The dynamic `--primary-foreground` colour token.
    public var primaryForeground: Color { self[.primaryForeground] }
    /// The dynamic `--secondary` colour token.
    public var secondary: Color { self[.secondary] }
    /// The dynamic `--secondary-foreground` colour token.
    public var secondaryForeground: Color { self[.secondaryForeground] }
    /// The dynamic `--muted` colour token.
    public var muted: Color { self[.muted] }
    /// The dynamic `--muted-foreground` colour token.
    public var mutedForeground: Color { self[.mutedForeground] }
    /// The dynamic `--accent` colour token.
    public var accent: Color { self[.accent] }
    /// The dynamic `--accent-foreground` colour token.
    public var accentForeground: Color { self[.accentForeground] }
    /// The dynamic `--destructive` colour token.
    public var destructive: Color { self[.destructive] }
    /// The dynamic `--border` colour token.
    public var border: Color { self[.border] }
    /// The dynamic `--input` colour token.
    public var input: Color { self[.input] }
    /// The dynamic `--ring` colour token.
    public var ring: Color { self[.ring] }
    /// The dynamic `--chart1` colour token.
    public var chart1: Color { self[.chart1] }
    /// The dynamic `--chart2` colour token.
    public var chart2: Color { self[.chart2] }
    /// The dynamic `--chart3` colour token.
    public var chart3: Color { self[.chart3] }
    /// The dynamic `--chart4` colour token.
    public var chart4: Color { self[.chart4] }
    /// The dynamic `--chart5` colour token.
    public var chart5: Color { self[.chart5] }
    /// The dynamic `--sidebar` colour token.
    public var sidebar: Color { self[.sidebar] }
    /// The dynamic `--sidebar-foreground` colour token.
    public var sidebarForeground: Color { self[.sidebarForeground] }
    /// The dynamic `--sidebar-primary` colour token.
    public var sidebarPrimary: Color { self[.sidebarPrimary] }
    /// The dynamic `--sidebar-primary-foreground` colour token.
    public var sidebarPrimaryForeground: Color { self[.sidebarPrimaryForeground] }
    /// The dynamic `--sidebar-accent` colour token.
    public var sidebarAccent: Color { self[.sidebarAccent] }
    /// The dynamic `--sidebar-accent-foreground` colour token.
    public var sidebarAccentForeground: Color { self[.sidebarAccentForeground] }
    /// The dynamic `--sidebar-border` colour token.
    public var sidebarBorder: Color { self[.sidebarBorder] }
    /// The dynamic `--sidebar-ring` colour token.
    public var sidebarRing: Color { self[.sidebarRing] }
    /// The dynamic `--surface` colour token.
    public var surface: Color { self[.surface] }
    /// The dynamic `--surface-raised` colour token.
    public var surfaceRaised: Color { self[.surfaceRaised] }
    /// The dynamic `--hairline` colour token.
    public var hairline: Color { self[.hairline] }
    /// The dynamic `--faint` colour token.
    public var faint: Color { self[.faint] }
    /// The dynamic `--iris` colour token.
    public var iris: Color { self[.iris] }
    /// The dynamic `--iris-soft` colour token.
    public var irisSoft: Color { self[.irisSoft] }
    /// The dynamic `--iris-foreground` colour token.
    public var irisForeground: Color { self[.irisForeground] }
    /// The dynamic `--iris-contrast` colour token.
    public var irisContrast: Color { self[.irisContrast] }
    /// The dynamic `--chip-time` colour token.
    public var chipTime: Color { self[.chipTime] }
    /// The dynamic `--chip-clipboard` colour token.
    public var chipClipboard: Color { self[.chipClipboard] }
    /// The dynamic `--chip-cursor` colour token.
    public var chipCursor: Color { self[.chipCursor] }
    /// The dynamic `--chip-form` colour token.
    public var chipForm: Color { self[.chipForm] }
    /// The dynamic `--chip-error` colour token.
    public var chipError: Color { self[.chipError] }
    /// The dynamic `--chip-neutral` colour token.
    public var chipNeutral: Color { self[.chipNeutral] }
    /// The dynamic `--success` colour token.
    public var success: Color { self[.success] }
    /// The dynamic `--warning` colour token.
    public var warning: Color { self[.warning] }
    /// The dynamic `--favorite` colour token.
    public var favorite: Color { self[.favorite] }
    /// The dynamic `--sidebar-vibrant` colour token.
    public var sidebarVibrant: Color { self[.sidebarVibrant] }
    /// The dynamic `--scrollbar-thumb` colour token.
    public var scrollbarThumb: Color { self[.scrollbarThumb] }
}

extension OKLCH {
    /// Alpha multiplied by `factor` (Tailwind's `/opacity` modifier).
    public func scaledAlpha(_ factor: Double) -> OKLCH { withAlpha(alpha * factor) }
}
