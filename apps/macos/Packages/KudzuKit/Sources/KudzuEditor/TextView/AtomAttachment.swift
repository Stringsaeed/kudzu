// Native attachments for the editor's atoms. Each one is a single U+FFFC character in the text
// storage, so caret movement, selection and deletion treat a chip or an image as one unit without
// any custom logic. The attachment carries the source it stands for; serializing the storage puts
// that source back.

import AppKit
import KudzuCore
import KudzuDesign

/// An `NSTextAttachment` standing for a source range (a chip or an image).
@MainActor
final class AtomAttachment: NSTextAttachment {
    /// The source text this character stands for (`{{date}}`, `[Logo]`).
    let source: String
    /// Help tag: the tag source, or "Not recognised: …" for verbatim tags.
    var tooltip: String?

    init(source: String, cell: NSTextAttachmentCell) {
        self.source = source
        super.init(data: nil, ofType: nil)
        attachmentCell = cell
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }
}

// MARK: Chip cell

/// The chip look from `globals.css` (`.kz-chip`): 20 pt tall, 7 pt padding, 1 pt margin, radius 6,
/// 11.5 pt medium sans (Paper Mono for error chips), tone at 13 % fill with a 1 pt inner ring at
/// 24 % (70 % on hover), the label baseline 1 pt above the text's.
@MainActor
final class ChipCell: NSTextAttachmentCell {
    nonisolated static let height: CGFloat = 20
    nonisolated static let padding: CGFloat = 7
    nonisolated static let margin: CGFloat = 1
    nonisolated static let radius: CGFloat = 6
    nonisolated static let fontSize: CGFloat = 11.5

    let label: String
    let tone: ChipTone

    init(label: String, tone: ChipTone) {
        self.label = label
        self.tone = tone
        super.init(imageCell: nil)
        setAccessibilityLabel("\(label) — click to edit")
    }

    @available(*, unavailable)
    required init(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    nonisolated var labelFont: NSFont {
        tone == .error
            ? KZFonts.monoNSFont(size: Self.fontSize, weight: .medium)
            : NSFont.systemFont(ofSize: Self.fontSize, weight: .medium)
    }

    nonisolated var toneToken: KZToken {
        switch tone {
        case .time: .chipTime
        case .clipboard: .chipClipboard
        case .cursor: .chipCursor
        case .form: .chipForm
        case .snippet: .chipNeutral
        case .error: .chipError
        }
    }

    private nonisolated var labelWidth: CGFloat {
        ceil((label as NSString).size(withAttributes: [.font: labelFont]).width)
    }

    override func cellSize() -> NSSize {
        NSSize(width: labelWidth + 2 * (Self.padding + Self.margin), height: Self.height)
    }

    /// Distance from the chip's bottom to its label's baseline (label centred, line-height 1).
    private nonisolated var labelBaseline: CGFloat {
        let font = labelFont
        return Self.height / 2 - (font.ascender + font.descender) / 2
    }

    /// The chip's baseline sits 1 pt above the line's (`vertical-align: 1px`).
    override func cellBaselineOffset() -> NSPoint {
        NSPoint(x: 0, y: 1 - labelBaseline)
    }

    override func cellFrame(for textContainer: NSTextContainer, proposedLineFragment lineFrag: NSRect, glyphPosition position: NSPoint, characterIndex charIndex: Int) -> NSRect {
        var size = cellSize()
        // Long verbatim tags never overflow the line; the label truncates instead.
        size.width = min(size.width, max(lineFrag.width - textContainer.lineFragmentPadding * 2, 24))
        return NSRect(origin: cellBaselineOffset(), size: size)
    }

    override func wantsToTrackMouse() -> Bool { false }

    override func draw(withFrame cellFrame: NSRect, in controlView: NSView?) {
        draw(withFrame: cellFrame, in: controlView, characterIndex: NSNotFound)
    }

    override func draw(withFrame cellFrame: NSRect, in controlView: NSView?, characterIndex charIndex: Int) {
        let hovered = (controlView as? KZTextView)?.hoveredAtomIndex == charIndex && charIndex != NSNotFound
        drawChip(in: cellFrame, hovered: hovered, flipped: controlView?.isFlipped ?? true)
    }

    override func draw(withFrame cellFrame: NSRect, in controlView: NSView?, characterIndex charIndex: Int, layoutManager: NSLayoutManager) {
        draw(withFrame: cellFrame, in: controlView, characterIndex: charIndex)
    }

    func drawChip(in frame: NSRect, hovered: Bool, flipped: Bool) {
        let box = frame.insetBy(dx: Self.margin, dy: 0)
        let fill = NSColor.kz.mix(toneToken, alpha: 0.13)
        let ring = NSColor.kz.mix(toneToken, alpha: hovered ? 0.7 : 0.24)
        let shape = NSBezierPath(roundedRect: box, xRadius: Self.radius, yRadius: Self.radius)
        fill.setFill()
        shape.fill()
        let inner = NSBezierPath(roundedRect: box.insetBy(dx: 0.5, dy: 0.5), xRadius: Self.radius - 0.5, yRadius: Self.radius - 0.5)
        inner.lineWidth = 1
        ring.setStroke()
        inner.stroke()

        let paragraph = NSMutableParagraphStyle()
        paragraph.lineBreakMode = .byTruncatingTail
        let font = labelFont
        let attributes: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: NSColor.kz[toneToken], .paragraphStyle: paragraph]
        let textHeight = font.ascender - font.descender
        // Baseline `labelBaseline` above the chip's bottom edge.
        let baselineFromTop = Self.height - labelBaseline
        let top = flipped ? box.minY + baselineFromTop - font.ascender : box.minY + labelBaseline + font.descender
        let rect = NSRect(x: box.minX + Self.padding, y: top, width: max(0, box.width - 2 * Self.padding), height: textHeight)
        (label as NSString).draw(with: rect, options: [.usesLineFragmentOrigin, .truncatesLastVisibleLine], attributes: attributes)
    }
}

// MARK: Image cell

/// An embedded image at its display width (scaled down to the line like `max-w-full`), rounded
/// (`rounded-sm`) and vertically centred on the text (`align-middle`).
@MainActor
final class ImageCell: NSTextAttachmentCell {
    let picture: NSImage?
    let dataUrl: String
    /// Height over width of the picture.
    let aspect: CGFloat
    let displayWidth: CGFloat
    let alt: String

    init(dataUrl: String, width: Int, alt: String) {
        picture = ImageCell.decode(dataUrl)
        let size = picture?.size ?? .zero
        aspect = size.width > 0 ? size.height / size.width : 1
        self.dataUrl = dataUrl
        displayWidth = CGFloat(width)
        self.alt = alt
        super.init(imageCell: nil)
        setAccessibilityLabel(alt)
    }

    @available(*, unavailable)
    required init(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    private static let cache = NSCache<NSString, NSImage>()

    static func decode(_ dataUrl: String) -> NSImage? {
        if let hit = cache.object(forKey: dataUrl as NSString) { return hit }
        guard let comma = dataUrl.firstIndex(of: ","), let data = Data(base64Encoded: String(dataUrl[dataUrl.index(after: comma)...])),
              let image = NSImage(data: data) else { return nil }
        cache.setObject(image, forKey: dataUrl as NSString)
        return image
    }


    override func cellSize() -> NSSize {
        NSSize(width: displayWidth, height: (displayWidth * aspect).rounded())
    }

    override func cellFrame(for textContainer: NSTextContainer, proposedLineFragment lineFrag: NSRect, glyphPosition position: NSPoint, characterIndex charIndex: Int) -> NSRect {
        let available = max(1, lineFrag.width - textContainer.lineFragmentPadding * 2)
        let width = min(displayWidth, available)
        let height = (width * aspect).rounded()
        let font = (textContainer.layoutManager?.textStorage?.attribute(.font, at: charIndex, effectiveRange: nil) as? NSFont) ?? .systemFont(ofSize: 13)
        // `vertical-align: middle`: the image's middle at the baseline plus half the x-height.
        return NSRect(x: 0, y: font.xHeight / 2 - height / 2, width: width, height: height)
    }

    override func wantsToTrackMouse() -> Bool { false }

    override func draw(withFrame cellFrame: NSRect, in controlView: NSView?) {
        NSGraphicsContext.saveGraphicsState()
        NSBezierPath(roundedRect: cellFrame, xRadius: Radius.sm, yRadius: Radius.sm).addClip()
        if let picture {
            picture.draw(in: cellFrame, from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: [.interpolation: NSImageInterpolation.high.rawValue])
        } else {
            NSColor.kz.muted.setFill()
            cellFrame.fill()
        }
        NSGraphicsContext.restoreGraphicsState()
    }

    override func draw(withFrame cellFrame: NSRect, in controlView: NSView?, characterIndex charIndex: Int) {
        draw(withFrame: cellFrame, in: controlView)
    }

    override func draw(withFrame cellFrame: NSRect, in controlView: NSView?, characterIndex charIndex: Int, layoutManager: NSLayoutManager) {
        draw(withFrame: cellFrame, in: controlView)
    }
}
