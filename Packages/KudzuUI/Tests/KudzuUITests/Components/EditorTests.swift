import AppKit
@testable import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct EditorTests {
    static let sample: [KzEditorSegment] = [
        .text("Hi "), .tag("{clipboard}"), .text("! Thanks for reaching out on "), .tag("{date:%A}"),
        .text(".\nI'll follow up at "), .tag("{time}"), .text(" from "), .tag("{app}"), .text("."),
    ]

    @Test func frame() throws {
        try snapshot("editor", width: 560, background: .surfaceRaised) {
            VStack(spacing: KzSpace.s5) {
                KzEditorFrame(minHeight: 120, insert: { PlaceholderButton(title: "+ Insert  ⌄", small: true) }) {
                    KzEditorTextView(Self.sample, isEditable: false)
                }
                KzEditorFrame(minHeight: 60, isFocused: true, insert: { PlaceholderButton(title: "+ Insert  ⌄", small: true) }) {
                    KzEditorTextView([.text("Focused: the frame shows a 2px "), .tag("{vine-ink}"), .text(" edge.")], isEditable: false)
                }
            }
        }
    }

    // MARK: Baseline

    /// Renders `text` in a TextKit 1 text view like the app editor's and returns its bitmap and scale.
    private func render(_ attributes: [NSAttributedString.Key: Any], text: String, width: CGFloat = 300, dark: Bool = false) -> (NSBitmapImageRep, CGFloat)? {
        let storage = NSTextStorage()
        let layoutManager = NSLayoutManager()
        storage.addLayoutManager(layoutManager)
        let container = NSTextContainer(size: NSSize(width: width, height: .greatestFiniteMagnitude))
        container.lineFragmentPadding = 0
        layoutManager.addTextContainer(container)
        let textView = NSTextView(frame: NSRect(x: 0, y: 0, width: width, height: 100), textContainer: container)
        textView.textContainerInset = .zero
        textView.drawsBackground = true
        textView.backgroundColor = .white
        textView.appearance = NSAppearance(named: .aqua)
        textView.textStorage?.setAttributedString(NSAttributedString(string: text, attributes: attributes))
        layoutManager.ensureLayout(for: container)
        guard let rep = textView.bitmapImageRepForCachingDisplay(in: textView.bounds) else { return nil }
        textView.cacheDisplay(in: textView.bounds, to: rep)
        return (rep, CGFloat(rep.pixelsHigh) / 100)
    }

    /// The last pixel row (from the top) that has dark ink, in points: the baseline of a line of capitals.
    private func inkBottom(_ rep: NSBitmapImageRep, scale: CGFloat) -> CGFloat? {
        var bottom = -1
        for y in 0..<rep.pixelsHigh {
            for x in 0..<rep.pixelsWide {
                if let c = rep.colorAt(x: x, y: y)?.usingColorSpace(.sRGB), c.brightnessComponent < 0.5 { bottom = y; break }
            }
        }
        return bottom < 0 ? nil : CGFloat(bottom + 1) / scale
    }

    /// The foundation flagged that `KzTextStyle.attributes`' baseline offset was untested in a real text view.
    /// With a fixed 22pt line TextKit puts all the extra space above the glyphs (baseline at 22 − descent);
    /// the offset must bring the baseline to where CSS half-leading puts it.
    @Test func baselineOffsetCentresGlyphsLikeCSS() throws {
        KzFonts.register()
        let style = KzTextStyle.content
        let font = style.nsFont
        let ascent = font.ascender, descent = -font.descender
        let lineHeight = style.spec.lineHeight
        // CSS: the line box is `lineHeight` tall and the glyph box (ascent + descent) is centred in it.
        let cssBaseline = (lineHeight - (ascent + descent)) / 2 + ascent

        let (rep, scale) = try #require(render(style.attributes(color: .black), text: "HHH"))
        let withOffset = try #require(inkBottom(rep, scale: scale))
        // One device pixel is the measurement resolution.
        #expect(abs(withOffset - cssBaseline) <= 1 / scale + 0.05, "baseline \(withOffset) vs CSS \(cssBaseline)")

        // And the offset is doing real work: without it the baseline is ~2pt too low.
        var plain = style.attributes(color: .black)
        plain[.baselineOffset] = nil
        let (rep2, scale2) = try #require(render(plain, text: "HHH"))
        let without = try #require(inkBottom(rep2, scale: scale2))
        #expect(without - cssBaseline > 1.5, "without the offset the baseline is \(without) vs CSS \(cssBaseline)")
        #expect(abs((lineHeight - descent) - without) <= 1 / scale2 + 0.05, "TextKit puts the baseline at lineHeight − descent when unadjusted")
    }

    @Test func tagMetrics() {
        KzFonts.register()
        let size = KzEditorTag.cellSize(forTitleWidth: 50)
        #expect(size == NSSize(width: 50 + 10 + 2, height: 20))
        // The cell hangs below the baseline (negative offset), by the text's descent plus half-leading, less the 1pt raise.
        let font = KzEditorTag.font
        let expected = 1 - ((20 - (font.ascender - font.descender)) / 2 - font.descender)
        #expect(abs(KzEditorTag.baselineOffset - expected) < 0.001)
        #expect(KzEditorTag.baselineOffset < 0)
        #expect(KzEditorStyle.containerInset == NSSize(width: 14, height: 12))
        #expect(KzEditorStyle.lineFragmentPadding == 0)
    }

    /// Draws a line with a tag between capitals, with the 22pt line box and CSS baseline marked, so the
    /// tag's alignment can be checked by eye, and checks the tag's measured position numerically.
    @Test func tagSitsOnTheLineLikeCSS() throws {
        KzFonts.register()
        let width: CGFloat = 320
        let textView = KzEditorTextView.makeTextView([.text("HH "), .tag("{date}"), .text(" HH")], width: width, isEditable: false)
        textView.appearance = NSAppearance(named: .aqua)
        textView.frame = NSRect(x: 0, y: 0, width: width, height: 60)
        let rep = try #require(textView.bitmapImageRepForCachingDisplay(in: textView.bounds))
        textView.cacheDisplay(in: textView.bounds, to: rep)
        let scale = CGFloat(rep.pixelsHigh) / 60

        let inset = KzEditorStyle.containerInset
        // Row range (in points from the text view's top, inside the line) where the tag fill appears.
        let fill = KzColorToken.vineSoft.value(dark: false)
        func isFill(_ c: NSColor) -> Bool {
            guard let c = c.usingColorSpace(.sRGB) else { return false }
            return abs(c.redComponent - fill.red) < 0.02 && abs(c.greenComponent - fill.green) < 0.02 && abs(c.blueComponent - fill.blue) < 0.02
        }
        var top = Int.max, bottom = -1
        for y in 0..<rep.pixelsHigh {
            for x in 0..<rep.pixelsWide where isFill(rep.colorAt(x: x, y: y) ?? .clear) { top = min(top, y); bottom = max(bottom, y); break }
        }
        #expect(bottom >= 0, "tag fill not found")
        let tagTop = CGFloat(top) / scale, tagBottom = CGFloat(bottom + 1) / scale
        #expect(abs((tagBottom - tagTop) - 20) <= 1 / scale + 0.05, "tag should be 20pt tall, was \(tagBottom - tagTop)")

        // The baseline in the text view: line top (inset) + CSS baseline. The tag's bottom hangs |offset| below it.
        let font = KzTextStyle.content.nsFont
        let cssBaseline = inset.height + (22 - (font.ascender - font.descender)) / 2 + font.ascender
        let expectedBottom = cssBaseline - KzEditorTag.baselineOffset
        #expect(abs(tagBottom - expectedBottom) <= 1 / scale + 0.5, "tag bottom \(tagBottom) vs expected \(expectedBottom)")

        // Review image: the real text view with the 22pt line box and CSS baseline overlaid.
        for dark in [false, true] {
            try snapshot("editor-baseline", width: width, background: .surfaceRaised) {
                ZStack(alignment: .topLeading) {
                    Image(nsImage: KzEditorTextView.image([.text("Hxg "), .tag("{date}"), .text(" Hxg")], size: CGSize(width: width, height: 60), dark: dark))
                    // 22pt line box
                    Rectangle().stroke(Color.red.opacity(0.35), lineWidth: 0.5).frame(width: width, height: 22).offset(y: inset.height)
                    // CSS baseline
                    Rectangle().fill(Color.blue.opacity(0.5)).frame(width: width, height: 0.5).offset(y: cssBaseline)
                }
                .frame(width: width, height: 60)
            }
            break
        }
    }
}
