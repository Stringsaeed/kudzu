// OKLCH colour values and their conversion to sRGB, implemented in-tree (no dependencies).
//
// `globals.css` writes every token in OKLCH. Browsers convert OKLCH → OKLab → linear sRGB →
// gamma-encoded sRGB and clip channels that fall outside [0, 1]; this file does the same with
// Björn Ottosson's published matrices, so a token drawn by SwiftUI matches the webview to
// within 1/255 per channel (see `KudzuDesignTests/ColorConversionTests`).

import Foundation

/// A colour in the OKLCH space: lightness `l` (0–1), chroma `c`, hue `h` in degrees, and alpha.
public struct OKLCH: Hashable, Sendable, CustomStringConvertible {
    /// OKLCH lightness, conventionally from zero to one.
    public var l: Double
    /// OKLCH chroma.
    public var c: Double
    /// Hue angle in degrees.
    public var h: Double
    /// Opacity from zero to one.
    public var alpha: Double

    /// Creates OKLCH with the supplied content and styling.
    public init(_ l: Double, _ c: Double, _ h: Double, alpha: Double = 1) {
        self.l = l
        self.c = c
        self.h = h
        self.alpha = alpha
    }

    /// Supporting text below the title.
    public var description: String {
        alpha == 1 ? "oklch(\(l) \(c) \(h))" : "oklch(\(l) \(c) \(h) / \(alpha))"
    }

    /// The same colour at a new alpha (CSS `oklch(l c h / a)`).
    public func withAlpha(_ alpha: Double) -> OKLCH { OKLCH(l, c, h, alpha: alpha) }

    // MARK: Conversion

    /// Gamma-encoded sRGB channels in [0, 1] (out-of-gamut channels clipped), alpha preserved.
    public var srgb: SRGB {
        let (r, g, b) = linearSRGB
        return SRGB(r: Self.encode(r), g: Self.encode(g), b: Self.encode(b), a: alpha)
    }

    /// Linear-light sRGB before clipping.
    public var linearSRGB: (Double, Double, Double) {
        let rad = h * .pi / 180
        let a = c * cos(rad)
        let b = c * sin(rad)
        // OKLab → LMS (non-linear), then cube.
        let l_ = l + 0.3963377774 * a + 0.2158037573 * b
        let m_ = l - 0.1055613458 * a - 0.0638541728 * b
        let s_ = l - 0.0894841775 * a - 1.2914855480 * b
        let L = l_ * l_ * l_
        let M = m_ * m_ * m_
        let S = s_ * s_ * s_
        // LMS → linear sRGB.
        return (
            +4.0767416621 * L - 3.3077115913 * M + 0.2309699292 * S,
            -1.2684380046 * L + 2.6097574011 * M - 0.3413193965 * S,
            -0.0041960863 * L - 0.7034186147 * M + 1.7076147010 * S
        )
    }

    /// sRGB transfer function (gamma encoding), then clip to [0, 1].
    static func encode(_ x: Double) -> Double {
        let sign: Double = x < 0 ? -1 : 1
        let v = abs(x)
        let y = v > 0.0031308 ? 1.055 * pow(v, 1 / 2.4) - 0.055 : 12.92 * v
        return min(1, max(0, sign * y))
    }

    /// Converts gamma-encoded sRGB back to OKLCH (used to mix with CSS keywords such as `white`).
    public init(srgb: SRGB) {
        func decode(_ x: Double) -> Double { x <= 0.04045 ? x / 12.92 : pow((x + 0.055) / 1.055, 2.4) }
        let r = decode(srgb.r), g = decode(srgb.g), b = decode(srgb.b)
        let L = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b
        let M = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b
        let S = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b
        let l_ = cbrt(L), m_ = cbrt(M), s_ = cbrt(S)
        let ok = 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_
        let oa = 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_
        let ob = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_
        let c = (oa * oa + ob * ob).squareRoot()
        var h = atan2(ob, oa) * 180 / .pi
        if h < 0 { h += 360 }
        self.init(ok, c, h, alpha: srgb.a)
    }

    // MARK: color-mix(in oklch, …)

    /// Chroma below which the hue is powerless and treated as missing (CSS Color 4 §12.2).
    static let achromatic = 1e-4

    /// `color-mix(in oklch, self (1 - amount), other amount)`: premultiplied alpha, shorter-arc
    /// hue interpolation, powerless (achromatic) hues taking the other colour's hue.
    public func mixed(with other: OKLCH, amount p: Double) -> OKLCH {
        let q = 1 - p
        var h1 = h, h2 = other.h
        let missing1 = c < Self.achromatic, missing2 = other.c < Self.achromatic
        if missing1 && !missing2 { h1 = h2 }
        if missing2 && !missing1 { h2 = h1 }
        var delta = (h2 - h1).truncatingRemainder(dividingBy: 360)
        if delta > 180 { delta -= 360 }
        if delta < -180 { delta += 360 }
        var hue = h1 + delta * p
        hue = hue.truncatingRemainder(dividingBy: 360)
        if hue < 0 { hue += 360 }
        let a = alpha * q + other.alpha * p
        guard a > 0 else { return OKLCH(0, 0, hue, alpha: 0) }
        let l = (self.l * alpha * q + other.l * other.alpha * p) / a
        let c = (self.c * alpha * q + other.c * other.alpha * p) / a
        return OKLCH(l, c, hue, alpha: a)
    }

    /// CSS `white` and `black` as OKLCH.
    public static let white = OKLCH(srgb: SRGB(r: 1, g: 1, b: 1, a: 1))
    /// The `black` design-system value.
    public static let black = OKLCH(0, 0, 0)
}

/// Gamma-encoded sRGB with alpha, each channel in [0, 1].
public struct SRGB: Hashable, Sendable {
    /// Gamma-encoded red channel.
    public var r: Double
    /// Gamma-encoded green channel.
    public var g: Double
    /// Gamma-encoded blue channel.
    public var b: Double
    /// Opacity from zero to one.
    public var a: Double

    /// Creates SRGB with the supplied content and styling.
    public init(r: Double, g: Double, b: Double, a: Double = 1) {
        self.r = r
        self.g = g
        self.b = b
        self.a = a
    }

    /// `#RRGGBB` with each channel rounded like a browser's 8-bit output.
    public var hex: String {
        func byte(_ v: Double) -> String { String(format: "%02X", Int((v * 255).rounded())) }
        return "#" + byte(r) + byte(g) + byte(b)
    }
}
