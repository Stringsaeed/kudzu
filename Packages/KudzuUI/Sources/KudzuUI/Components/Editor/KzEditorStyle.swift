import AppKit
import SwiftUI

/// Everything the app's `NSTextView` chip editor needs to look like the design system's Editor.
///
/// The editor itself stays in the app (`Kudzu/Editor/TemplateEditor/`); this type only supplies the
/// values and drawing, so adopting the look is: apply `contentAttributes` to literal text, call
/// `configure(_:)` on the text view, and draw tag attachments with `KzEditorTag`.
public enum KzEditorStyle {
    /// The text style of the user's own words: `content`, 14/22.
    public static let textStyle: KzTextStyle = .content

    /// The editor body's padding: `space-3` (12) vertically and 14 horizontally, as `NSTextView.textContainerInset`.
    public static let containerInset = NSSize(width: 14, height: KzSpace.s3)

    /// Must be 0 so `containerInset` is the whole horizontal padding (the default adds 5pt per side).
    public static let lineFragmentPadding: CGFloat = 0

    /// The body's minimum height (140).
    public static let minHeight: CGFloat = 140

    /// Attributes for literal text: Geist 14, fixed 22pt lines, and the baseline offset that centres
    /// the glyphs in the line the way CSS half-leading does. Works in TextKit 1 and 2.
    public static func contentAttributes(color: NSColor = KzColorToken.ink.nsColor) -> [NSAttributedString.Key: Any] {
        textStyle.attributes(color: color)
    }

    /// Typing attributes for the text view, so text typed after a tag or into an empty editor matches.
    public static func typingAttributes(color: NSColor = KzColorToken.ink.nsColor) -> [NSAttributedString.Key: Any] {
        contentAttributes(color: color)
    }

    /// Applies the body's metrics to a text view: container inset, no line-fragment padding, a clear
    /// background (the frame draws the fill), the `ink` caret and typing attributes.
    @MainActor
    public static func configure(_ textView: NSTextView) {
        textView.textContainerInset = containerInset
        textView.textContainer?.lineFragmentPadding = lineFragmentPadding
        textView.drawsBackground = false
        textView.insertionPointColor = KzColorToken.ink.nsColor
        textView.font = textStyle.nsFont
        textView.typingAttributes = typingAttributes()
    }
}

/// The inline tag look: `code` mono in `vine-ink` on a `vine-soft` rounded rectangle (`radius-xs`),
/// 20pt tall, 5pt side padding, 1pt side margin, raised 1pt off the text baseline.
///
/// Use it from an `NSTextAttachmentCell`: report `cellSize(forTitleWidth:)` and `cellBaselineOffset`,
/// and call `draw(_:in:isSelected:)` from `draw(withFrame:in:)`. `KzEditorTagCell` is a ready-made cell.
public enum KzEditorTag {
    /// Side padding inside the tag.
    public static let horizontalPadding: CGFloat = 5
    /// Space outside the tag on each side.
    public static let horizontalMargin: CGFloat = 1
    /// Tag height (the CSS line box of the tag).
    public static let height: CGFloat = 20
    /// How far the tag is raised above the text baseline.
    public static let verticalAlign: CGFloat = 1
    /// `radius-xs`.
    public static let cornerRadius: CGFloat = KzRadius.xs

    /// Geist Mono 12 Medium, the weight the design system's tag uses (the `code` token is Regular).
    public static var font: NSFont {
        let spec = KzTextStyle.code.spec
        let name = KzFonts.postScriptName(family: .mono, weight: 500)
        if KzFonts.isAvailable(name), let font = NSFont(name: name, size: spec.size) { return font }
        return .monospacedSystemFont(ofSize: spec.size, weight: .medium)
    }

    /// Text colour: `vine-ink`.
    public static var ink: NSColor { KzColorToken.vineInk.nsColor }
    /// Fill: `vine-soft`.
    public static var fill: NSColor { KzColorToken.vineSoft.nsColor }
    /// Fill of a selected tag: `vine-ink`, so a selected tag is clearly a unit.
    public static var selectedFill: NSColor { KzColorToken.vineInk.nsColor }
    /// Text on a selected tag: `surface-raised`, which reads on `vine-ink` in both themes.
    public static var selectedInk: NSColor { KzColorToken.surfaceRaised.nsColor }

    /// Attributes for the tag's title.
    public static func titleAttributes(isSelected: Bool = false) -> [NSAttributedString.Key: Any] {
        [.font: font, .foregroundColor: isSelected ? selectedInk : ink]
    }

    /// The width of `title` set in the tag font.
    public static func titleWidth(_ title: String) -> CGFloat {
        ceil((title as NSString).size(withAttributes: [.font: font]).width)
    }

    /// The attachment cell size for a title of the given width: padding and margin included.
    public static func cellSize(forTitleWidth width: CGFloat) -> NSSize {
        NSSize(width: width + 2 * horizontalPadding + 2 * horizontalMargin, height: height)
    }

    /// The distance from the baseline to the cell's bottom edge (`cellBaselineOffset.y`).
    ///
    /// CSS centres the 12pt text in the 20pt box (half-leading) and then raises the box by
    /// `verticalAlign`. The text baseline is `(20 − (ascent + descent)) / 2 + descent` above the box
    /// bottom, so the box bottom is that far below the line's baseline, less the 1pt raise. The result
    /// is negative: the cell hangs below the baseline.
    public static var baselineOffset: CGFloat {
        let font = font
        let ascent = font.ascender
        let descent = -font.descender
        let baselineAboveBottom = (height - (ascent + descent)) / 2 + descent
        return verticalAlign - baselineAboveBottom
    }

    /// Draws a tag filling `frame` (a cell frame, margins included): rounded fill and centred title.
    @MainActor
    public static func draw(_ title: String, in frame: NSRect, isSelected: Bool = false, appearance: NSAppearance? = nil) {
        (appearance ?? NSAppearance.currentDrawing()).performAsCurrentDrawingAppearance {
            let box = NSRect(x: frame.minX + horizontalMargin, y: frame.midY - height / 2,
                             width: frame.width - 2 * horizontalMargin, height: height)
            (isSelected ? selectedFill : fill).setFill()
            NSBezierPath(roundedRect: box, xRadius: cornerRadius, yRadius: cornerRadius).fill()
            let font = font
            let textHeight = font.ascender - font.descender
            // Centring the glyph box matches CSS half-leading, in flipped and unflipped views alike.
            (title as NSString).draw(
                at: NSPoint(x: box.minX + horizontalPadding, y: box.midY - textHeight / 2),
                withAttributes: titleAttributes(isSelected: isSelected)
            )
        }
    }
}

/// A ready-made attachment cell that draws a `KzEditorTag`. The app's chip cell can subclass or copy it.
public final class KzEditorTagCell: NSTextAttachmentCell {
    /// The text shown in the tag, e.g. `{date:%d %b %Y}`.
    public let tagText: String

    /// Creates a cell for `title`.
    public init(title: String) {
        self.tagText = title
        super.init(textCell: "")
    }

    required init(coder: NSCoder) {
        fatalError("KzEditorTagCell is not archived")
    }

    // The cell protocol's layout queries are nonisolated; TextKit calls them on the main thread.
    nonisolated override public func cellSize() -> NSSize {
        MainActor.assumeIsolated { KzEditorTag.cellSize(forTitleWidth: KzEditorTag.titleWidth(tagText)) }
    }

    nonisolated override public func cellBaselineOffset() -> NSPoint {
        MainActor.assumeIsolated { NSPoint(x: 0, y: KzEditorTag.baselineOffset) }
    }

    public override func draw(withFrame cellFrame: NSRect, in controlView: NSView?) {
        KzEditorTag.draw(tagText, in: cellFrame, isSelected: false, appearance: controlView?.effectiveAppearance)
    }

    public override func draw(withFrame cellFrame: NSRect, in controlView: NSView?, characterIndex: Int, layoutManager: NSLayoutManager) {
        var selected = false
        if let textView = controlView as? NSTextView {
            selected = textView.selectedRanges.contains { NSLocationInRange(characterIndex, $0.rangeValue) }
        }
        KzEditorTag.draw(tagText, in: cellFrame, isSelected: selected, appearance: controlView?.effectiveAppearance)
    }
}
