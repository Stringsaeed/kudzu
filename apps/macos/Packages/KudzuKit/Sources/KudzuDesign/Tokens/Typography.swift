// Type scale. SF Pro (system) for UI (`--app-sans: -apple-system, …`), Paper Mono for triggers and
// templates (`--app-mono: "Paper Mono", ui-monospace, "SF Mono", …`). The body is 13 px on a 20 px
// line (`text-[13px] leading-5`); shadcn primitives use Tailwind's `text-sm` (14/20) and `text-xs`
// (12/16). CSS px map 1:1 to points.

import AppKit
import SwiftUI

/// A CSS text style: size, line height, weight, family and letter spacing (in em).
public struct KZTextStyle: Hashable, Sendable {
    /// The system sans or bundled monospaced typeface.
    public enum Family: Hashable, Sendable { case sans, mono }

    /// The point size or spacing variant.
    public var size: CGFloat
    /// The intended text line height in points.
    public var lineHeight: CGFloat
    /// The native font weight.
    public var weight: Font.Weight
    /// The sans or mono font family.
    public var family: Family
    /// `letter-spacing` in em (`tracking-wide` = 0.025, `tracking-tight` = -0.025).
    public var tracking: CGFloat
    /// `font-variant-numeric: tabular-nums` (`.tabular`).
    public var tabular: Bool

    /// Creates Family with the supplied content and styling.
    public init(size: CGFloat, lineHeight: CGFloat, weight: Font.Weight = .regular, family: Family = .sans, tracking: CGFloat = 0, tabular: Bool = false) {
        self.size = size
        self.lineHeight = lineHeight
        self.weight = weight
        self.family = family
        self.tracking = tracking
        self.tabular = tabular
    }

    /// Returns a copy with the requested font weight.
    public func weight(_ w: Font.Weight) -> Self { var s = self; s.weight = w; return s }
    /// Returns a copy with the requested line height.
    public func leading(_ lh: CGFloat) -> Self { var s = self; s.lineHeight = lh; return s }
    /// Line height as a multiple of the size (`leading-none` = 1, `leading-snug` = 1.375).
    public func leading(multiple m: CGFloat) -> Self { leading(size * m) }
    /// Returns a copy with the requested letter spacing.
    public func tracking(_ em: CGFloat) -> Self { var s = self; s.tracking = em; return s }
    /// Returns a monospaced font or text style.
    public func mono() -> Self { var s = self; s.family = .mono; return s }
    /// Returns a style with fixed-width numeral spacing.
    public func tabularNumbers() -> Self { var s = self; s.tabular = true; return s }

    /// The `font` design-system value.
    public var font: Font {
        let f = family == .mono ? KZFonts.mono(size: size, weight: weight) : Font.system(size: size, weight: weight)
        return tabular ? f.monospacedDigit() : f
    }

    /// The `nsFont` design-system value.
    public var nsFont: NSFont {
        let base = family == .mono ? KZFonts.monoNSFont(size: size, weight: weight) : NSFont.systemFont(ofSize: size, weight: weight.nsWeight)
        return base
    }

    /// Half-leading added above and below each line so the line box is `lineHeight` tall.
    public var halfLeading: CGFloat {
        let f = nsFont
        let natural = f.ascender - f.descender + f.leading
        return max(0, (lineHeight - natural) / 2)
    }

    // MARK: Scale

    /// Body: `text-[13px] leading-5`.
    public static let body = KZTextStyle(size: 13, lineHeight: 20)
    /// `text-xs` (12 / 16).
    public static let xs = KZTextStyle(size: 12, lineHeight: 16)
    /// `text-sm` (14 / 20).
    public static let sm = KZTextStyle(size: 14, lineHeight: 20)
    /// `text-base` (16 / 24).
    public static let base = KZTextStyle(size: 16, lineHeight: 24)
    /// `text-lg` (18 / 28).
    public static let lg = KZTextStyle(size: 18, lineHeight: 28)
    /// `text-xl` (20 / 28).
    public static let xl = KZTextStyle(size: 20, lineHeight: 28)
    /// `text-2xl` (24 / 32).
    public static let xl2 = KZTextStyle(size: 24, lineHeight: 32)
    /// `text-[11px]` labels and counts.
    public static let label11 = KZTextStyle(size: 11, lineHeight: 16)
    /// `text-[0.8rem]` (small buttons and toggles).
    public static let small = KZTextStyle(size: 12.8, lineHeight: 20)
    /// Insight tiles: 26 px figures.
    public static let insight = KZTextStyle(size: 26, lineHeight: 32, weight: .semibold, tabular: true)
    /// Template editor: 13 px Paper Mono at 1.7 line height.
    public static let editor = KZTextStyle(size: 13, lineHeight: 13 * 1.7, family: .mono)
}

/// Letter-spacing helpers (`tracking-*`), in em.
public enum Tracking {
    /// Extra-tight letter spacing, in em.
    public static let tighter: CGFloat = -0.05
    /// Tight letter spacing, in em.
    public static let tight: CGFloat = -0.025
    /// The display tracking used on headings (`tracking-[-0.02em]`).
    public static let display: CGFloat = -0.02
    /// Zero letter spacing.
    public static let normal: CGFloat = 0
    /// Wide letter spacing, in em.
    public static let wide: CGFloat = 0.025
    /// Extra-wide letter spacing, in em.
    public static let wider: CGFloat = 0.05
    /// Maximum label spacing, in em.
    public static let widest: CGFloat = 0.1

    /// Points for an em value at a size.
    public static func points(_ em: CGFloat, size: CGFloat) -> CGFloat { em * size }
}

// MARK: Fonts

/// System and Paper Mono font selection.
public enum KZFonts {
    /// Paper Mono's family name; the app target registers `PaperMono-Variable.ttf`.
    public static let monoFamily = "Paper Mono"

    /// Whether Paper Mono is registered in this process.
    public static var hasPaperMono: Bool {
        NSFontManager.shared.availableMembers(ofFontFamily: monoFamily) != nil
    }

    /// Paper Mono, falling back to SF Mono (`ui-monospace`) like the CSS stack.
    public static func mono(size: CGFloat, weight: Font.Weight = .regular) -> Font {
        hasPaperMono ? Font.custom(monoFamily, size: size).weight(weight) : Font.system(size: size, weight: weight, design: .monospaced)
    }

    /// Returns Paper Mono or the system monospaced fallback.
    public static func monoNSFont(size: CGFloat, weight: Font.Weight = .regular) -> NSFont {
        if hasPaperMono, let f = NSFontManager.shared.font(withFamily: monoFamily, traits: [], weight: weight.appKitWeightClass, size: size) {
            return f
        }
        return NSFont.monospacedSystemFont(ofSize: size, weight: weight.nsWeight)
    }
}

extension Font {
    /// Kudzu fonts: `Font.kz.body`, `Font.kz.mono`, `Font.kz.sans(13, .medium)`.
    public static let kz = KZFontSet()
}

/// Convenience fonts for native view content.
public struct KZFontSet: Sendable {
    /// 13 px SF Pro.
    public var body: Font { .system(size: 13) }
    /// 12 px (`text-xs`).
    public var xs: Font { .system(size: 12) }
    /// 14 px (`text-sm`).
    public var sm: Font { .system(size: 14) }
    /// 11 px secondary labels.
    public var label: Font { .system(size: 11) }
    /// 13 px Paper Mono.
    public var mono: Font { KZFonts.mono(size: 13) }
    /// Returns a monospaced font or text style.
    public func mono(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font { KZFonts.mono(size: size, weight: weight) }
    /// Returns a system sans font at the requested size and weight.
    public func sans(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font { .system(size: size, weight: weight) }
}

extension Font.Weight {
    var nsWeight: NSFont.Weight {
        switch self {
        case .ultraLight: .ultraLight
        case .thin: .thin
        case .light: .light
        case .medium: .medium
        case .semibold: .semibold
        case .bold: .bold
        case .heavy: .heavy
        case .black: .black
        default: .regular
        }
    }

    /// NSFontManager weight classes (5 = regular, 6 = medium, 8 = semibold, 9 = bold).
    var appKitWeightClass: Int {
        switch self {
        case .ultraLight: 2
        case .thin: 3
        case .light: 4
        case .medium: 6
        case .semibold: 8
        case .bold: 9
        case .heavy: 10
        case .black: 11
        default: 5
        }
    }
}

// MARK: Modifier

extension View {
    /// Applies a CSS text style: font, letter spacing, and line height (lines spaced to
    /// `lineHeight`, with the half-leading above the first and below the last line).
    public func textStyle(_ style: KZTextStyle) -> some View {
        let f = style.nsFont
        let natural = f.ascender - f.descender + f.leading
        return self
            .font(style.font)
            .tracking(style.tracking * style.size)
            .lineSpacing(max(0, style.lineHeight - natural))
            .padding(.vertical, style.halfLeading)
    }
}
