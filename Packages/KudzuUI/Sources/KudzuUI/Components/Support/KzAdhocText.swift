import AppKit
import SwiftUI

extension View {
    /// Sets text that the design system's CSS specifies but no type token covers (for example the
    /// 500 12/16 stat label). Prefer `.kzTextStyle` whenever a token fits.
    func kzText(
        _ spec: KzTextSpec,
        matchLineBox: Bool = true
    ) -> some View {
        modifier(KzAdhocTextModifier(spec: spec, matchLineBox: matchLineBox))
    }
}

private struct KzAdhocTextModifier: ViewModifier {
    let spec: KzTextSpec
    let matchLineBox: Bool

    func body(content: Content) -> some View {
        let name = KzFonts.postScriptName(family: spec.family, weight: spec.weight)
        let bundled = KzFonts.isAvailable(name)
        let nsFont = bundled ? NSFont(name: name, size: spec.size) : nil
        let natural: CGFloat = nsFont.map { $0.ascender - $0.descender + $0.leading } ?? spec.size * 1.2
        let font: Font = if bundled {
            .custom(name, fixedSize: spec.size)
        } else {
            .system(size: spec.size, weight: spec.weight >= 550 ? .semibold : spec.weight >= 450 ? .medium : .regular,
                    design: spec.family == .mono ? .monospaced : .default)
        }
        content
            .font(spec.tabularFigures ? font.monospacedDigit() : font)
            .tracking(spec.tracking)
            .lineSpacing(max(0, spec.lineHeight - natural))
            .padding(.vertical, matchLineBox ? (spec.lineHeight - natural) / 2 : 0)
    }
}

extension KzTextSpec {
    /// A spec for CSS values that no token covers.
    static func adhoc(
        family: KzFontFamily = .sans, size: CGFloat, lineHeight: CGFloat, weight: Int,
        trackingEm: CGFloat = 0, tabular: Bool = false
    ) -> KzTextSpec {
        KzTextSpec(family: family, size: size, lineHeight: lineHeight, weight: weight, trackingEm: trackingEm,
                   tabularFigures: tabular, group: "Adhoc", sample: "")
    }
}
