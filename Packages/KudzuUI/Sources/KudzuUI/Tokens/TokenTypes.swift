import AppKit
import SwiftUI

/// A colour value in sRGB with straight alpha, as written in tokens.json.
public struct KzRGBA: Hashable, Sendable {
    /// Red, 0…1.
    public var red: Double
    /// Green, 0…1.
    public var green: Double
    /// Blue, 0…1.
    public var blue: Double
    /// Opacity, 0…1.
    public var alpha: Double

    /// Creates a colour from 0…1 components.
    public init(_ red: Double, _ green: Double, _ blue: Double, _ alpha: Double = 1) {
        self.red = red
        self.green = green
        self.blue = blue
        self.alpha = alpha
    }

    /// Creates a colour from 0…255 components, the way the generator writes hex values.
    public init(_ red: UInt8, _ green: UInt8, _ blue: UInt8, alpha: UInt8 = 0xFF) {
        self.init(Double(red) / 255, Double(green) / 255, Double(blue) / 255, Double(alpha) / 255)
    }

    /// The same colour with a different opacity.
    public func withAlpha(_ alpha: Double) -> KzRGBA {
        KzRGBA(red, green, blue, alpha)
    }

    /// `#rrggbb`, or `#rrggbbaa` when not opaque, matching tokens.json.
    public var hex: String {
        func byte(_ v: Double) -> String { String(format: "%02x", Int((v * 255).rounded())) }
        let rgb = "#" + byte(red) + byte(green) + byte(blue)
        return alpha >= 0.999 ? rgb : rgb + byte(alpha)
    }

    /// A static (non-dynamic) AppKit colour.
    public var nsColor: NSColor {
        NSColor(srgbRed: red, green: green, blue: blue, alpha: alpha)
    }

    /// A static (non-dynamic) SwiftUI colour; use `KzColor` for theme-aware colours.
    public var color: Color {
        Color(.sRGB, red: red, green: green, blue: blue, opacity: alpha)
    }
}

extension KzColorToken {
    /// A dynamic AppKit colour that resolves Light, Dark and Increase Contrast from the drawing appearance.
    public var nsColor: NSColor {
        dynamicColor(forceOpaque: false)
    }

    /// A dynamic SwiftUI colour bridged from `nsColor`.
    public var color: Color {
        Color(nsColor: nsColor)
    }

    /// The token as a dynamic colour with full opacity (Reduce Transparency fallback for glass).
    public var opaqueColor: Color {
        Color(nsColor: dynamicColor(forceOpaque: true))
    }

    /// The value for a given appearance, applying the Increase Contrast substitutions.
    public func value(dark: Bool, highContrast: Bool = false) -> KzRGBA {
        if highContrast, let substitute = highContrastSubstitute {
            return substitute.value(dark: dark, highContrast: false)
        }
        let base = dark ? self.dark : light
        return highContrast && isOpaqueInHighContrast ? base.withAlpha(1) : base
    }

    private func dynamicColor(forceOpaque: Bool) -> NSColor {
        let token = self
        return NSColor(name: NSColor.Name("kz." + rawValue + (forceOpaque ? ".opaque" : ""))) { appearance in
            let match = appearance.bestMatch(from: [
                .aqua, .darkAqua,
                .accessibilityHighContrastAqua, .accessibilityHighContrastDarkAqua,
            ])
            let dark = match == .darkAqua || match == .accessibilityHighContrastDarkAqua
            let highContrast = match == .accessibilityHighContrastAqua || match == .accessibilityHighContrastDarkAqua
            var rgba = token.value(dark: dark, highContrast: highContrast)
            if forceOpaque { rgba.alpha = 1 }
            return rgba.nsColor
        }
    }
}

/// A named length token (spacing, radius, size or blur) with its usage note.
public struct KzMetric: Hashable, Sendable, Identifiable {
    /// The CSS token name, e.g. `space-4`.
    public let token: String
    /// The Swift member name, e.g. `s4`.
    public let name: String
    /// The value in points.
    public let value: CGFloat
    /// How the design system says this token should be used.
    public let usage: String
    /// Stable identity for lists.
    public var id: String { token }

    /// Creates a metric; the generator is the only expected caller.
    public init(token: String, name: String, value: CGFloat, usage: String) {
        self.token = token
        self.name = name
        self.value = value
        self.usage = usage
    }
}

/// The two type families: Geist for UI text, Geist Mono for triggers and literal text.
public enum KzFontFamily: String, Sendable, CaseIterable {
    /// Geist.
    case sans
    /// Geist Mono.
    case mono
}

/// The raw numbers behind a `KzTextStyle`.
public struct KzTextSpec: Hashable, Sendable {
    /// Which family the style uses.
    public let family: KzFontFamily
    /// Font size in points.
    public let size: CGFloat
    /// CSS line height in points (the height of one line box).
    public let lineHeight: CGFloat
    /// CSS font weight: 400, 500 or 600.
    public let weight: Int
    /// Letter spacing in em; multiply by `size` for points.
    public let trackingEm: CGFloat
    /// Whether digits should be tabular (equal width).
    public let tabularFigures: Bool
    /// The design-system group the style belongs to (Display, Text, Mono).
    public let group: String
    /// Sample text from the design system.
    public let sample: String

    /// Letter spacing in points.
    public var tracking: CGFloat { trackingEm * size }

    /// Creates a spec; the generator is the only expected caller.
    public init(family: KzFontFamily, size: CGFloat, lineHeight: CGFloat, weight: Int, trackingEm: CGFloat, tabularFigures: Bool, group: String, sample: String) {
        self.family = family
        self.size = size
        self.lineHeight = lineHeight
        self.weight = weight
        self.trackingEm = trackingEm
        self.tabularFigures = tabularFigures
        self.group = group
        self.sample = sample
    }
}

/// One layer of a CSS `box-shadow` list.
public struct KzShadowLayer: Hashable, Sendable {
    /// `inset` shadows draw inside the shape (the glass top highlight); others drop outside it.
    public let inset: Bool
    /// Horizontal offset in points.
    public let x: CGFloat
    /// Vertical offset in points (positive is down).
    public let y: CGFloat
    /// CSS blur radius in points (twice the Gaussian sigma).
    public let blur: CGFloat
    /// CSS spread in points; negative shrinks the shadow.
    public let spread: CGFloat
    /// Shadow colour.
    public let color: KzRGBA

    /// Creates a layer; the generator is the only expected caller.
    public init(inset: Bool, x: CGFloat, y: CGFloat, blur: CGFloat, spread: CGFloat, color: KzRGBA) {
        self.inset = inset
        self.x = x
        self.y = y
        self.blur = blur
        self.spread = spread
        self.color = color
    }
}

extension KzShadow {
    /// The layers for a colour scheme.
    public func layers(for scheme: ColorScheme) -> [KzShadowLayer] {
        scheme == .dark ? darkLayers : lightLayers
    }
}
