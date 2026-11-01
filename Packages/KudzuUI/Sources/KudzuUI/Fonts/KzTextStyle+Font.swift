import AppKit
import CoreText
import SwiftUI

extension KzTextStyle {
    /// The PostScript name of the face this style uses.
    public var postScriptName: String {
        KzFonts.postScriptName(family: spec.family, weight: spec.weight)
    }

    /// Whether the bundled face is in use (false means the system-font fallback).
    public var usesBundledFont: Bool {
        KzFonts.isAvailable(postScriptName)
    }

    /// The style as an AppKit font, with tabular figures where the style asks for them.
    public var nsFont: NSFont {
        let spec = spec
        var font: NSFont
        if usesBundledFont, let custom = NSFont(name: postScriptName, size: spec.size) {
            font = custom
        } else {
            let weight: NSFont.Weight = spec.weight >= 550 ? .semibold : spec.weight >= 450 ? .medium : .regular
            font = spec.family == .mono
                ? .monospacedSystemFont(ofSize: spec.size, weight: weight)
                : .systemFont(ofSize: spec.size, weight: weight)
        }
        if spec.tabularFigures {
            let descriptor = font.fontDescriptor.addingAttributes([
                .featureSettings: [[
                    NSFontDescriptor.FeatureKey.typeIdentifier: kNumberSpacingType,
                    NSFontDescriptor.FeatureKey.selectorIdentifier: kMonospacedNumbersSelector,
                ]],
            ])
            font = NSFont(descriptor: descriptor, size: spec.size) ?? font
        }
        return font
    }

    /// The style as a SwiftUI font (face, size and figures; use `.kzTextStyle(_:)` for line height and tracking too).
    public var font: Font {
        let spec = spec
        var font: Font
        if usesBundledFont {
            font = .custom(postScriptName, fixedSize: spec.size)
        } else {
            let weight: Font.Weight = spec.weight >= 550 ? .semibold : spec.weight >= 450 ? .medium : .regular
            font = .system(size: spec.size, weight: weight, design: spec.family == .mono ? .monospaced : .default)
        }
        return spec.tabularFigures ? font.monospacedDigit() : font
    }

    /// The font's natural line height (ascent + descent + leading) in points.
    public var naturalLineHeight: CGFloat {
        let font = nsFont
        return font.ascender - font.descender + font.leading
    }

    /// Extra space SwiftUI should add between lines to reach the CSS line height.
    public var lineSpacing: CGFloat {
        max(0, spec.lineHeight - naturalLineHeight)
    }

    /// A paragraph style with a fixed line height, for AppKit text views.
    public var paragraphStyle: NSParagraphStyle {
        let style = NSMutableParagraphStyle()
        style.minimumLineHeight = spec.lineHeight
        style.maximumLineHeight = spec.lineHeight
        return style
    }

    /// Attributed-string attributes for AppKit text (font, kerning, line height and colour).
    ///
    /// With a fixed line height TextKit puts the extra space above the glyphs; the baseline offset
    /// moves half of it below, centring the glyphs the way CSS half-leading does.
    public func attributes(color: NSColor = KzColorToken.ink.nsColor) -> [NSAttributedString.Key: Any] {
        let font = nsFont
        let natural = font.ascender - font.descender
        return [
            .font: font,
            .kern: spec.tracking,
            .paragraphStyle: paragraphStyle,
            .foregroundColor: color,
            .baselineOffset: max(0, spec.lineHeight - natural) / 2,
        ]
    }
}

/// Applies a Kudzu type style: face, size, CSS line height, tracking and figures.
private struct KzTextStyleModifier: ViewModifier {
    let style: KzTextStyle
    let matchLineBox: Bool

    func body(content: Content) -> some View {
        let spec = style.spec
        // SwiftUI only adds line spacing *between* lines; CSS also adds half the leading
        // above and below. Padding the leaf by that half-leading reproduces the CSS box.
        let halfLeading = (spec.lineHeight - style.naturalLineHeight) / 2
        content
            .font(style.font)
            .tracking(spec.tracking)
            .lineSpacing(style.lineSpacing)
            .padding(.vertical, matchLineBox ? halfLeading : 0)
    }
}

extension View {
    /// Sets a Kudzu type style: face, size, line height, tracking and tabular figures.
    ///
    /// With `matchLineBox` (the default) a single line is exactly `lineHeight` tall, as in the CSS
    /// reference. Apply it to the `Text` itself rather than a container so each line box matches.
    public func kzTextStyle(_ style: KzTextStyle, matchLineBox: Bool = true) -> some View {
        modifier(KzTextStyleModifier(style: style, matchLineBox: matchLineBox))
    }
}
