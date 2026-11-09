// Scheme-resolved token colours for styles (the `dark:` variants need the scheme anyway) and the
// text colour utilities.

import SwiftUI

/// Token colours for one colour scheme, as fixed sRGB colours.
public struct KZTheme: Sendable {
    /// Surface recipe for dark appearance.
    public var dark: Bool

    /// Creates KZTheme with the supplied content and styling.
    public init(_ scheme: ColorScheme) { dark = scheme == .dark }
    /// Creates KZTheme with the supplied content and styling.
    public init(dark: Bool) { self.dark = dark }

    /// Returns the OKLCH token value for this appearance.
    public func value(_ token: KZToken) -> OKLCH { Palette.value(token, dark: dark) }

    /// `bg-<token>` / `text-<token>`.
    public subscript(_ token: KZToken) -> Color { Color.oklch(value(token)) }
    /// `bg-<token>/<alpha>`.
    public subscript(_ token: KZToken, alpha alpha: Double) -> Color { Color.oklch(value(token).scaledAlpha(alpha)) }
    /// `color-mix(in oklch, a, b amount)`.
    public func mixed(_ a: KZToken, _ b: KZToken, _ amount: Double) -> Color { Color.oklch(value(a).mixed(with: value(b), amount: amount)) }
    /// `color-mix(in oklch, a, white amount)`.
    public func mixedWhite(_ a: KZToken, _ amount: Double) -> Color { Color.oklch(value(a).mixed(with: .white, amount: amount)) }
    /// `bg-black/<alpha>`.
    public static func black(_ alpha: Double) -> Color { Color.oklch(OKLCH(0, 0, 0, alpha: alpha)) }
}

/// `text-*` colour utilities.
public enum TextTone: Sendable {
    case foreground, muted, faint, iris, destructive, success, warning, primary, popover

    /// The `token` design-system value.
    public var token: KZToken {
        switch self {
        case .foreground: .foreground
        case .muted: .mutedForeground
        case .faint: .faint
        case .iris: .irisForeground
        case .destructive: .destructive
        case .success: .success
        case .warning: .warning
        case .primary: .primary
        case .popover: .popoverForeground
        }
    }
}

extension View {
    /// `text-foreground`, `text-muted-foreground`, `text-faint`, …
    public func textTone(_ tone: TextTone) -> some View { foregroundStyle(TokenFill(tone.token)) }
    /// `text-muted-foreground`.
    public func mutedForeground() -> some View { textTone(.muted) }
    /// `text-faint`.
    public func faint() -> some View { textTone(.faint) }
}
