import AppKit
import KudzuCore

/// A text attachment that stands for exactly one template tag.
///
/// Attachments are immutable with respect to their tag: edits replace the attachment, so
/// `NSTextView`'s undo stack (which stores attributed substrings) restores the old tag.
final class ChipAttachment: NSTextAttachment {
    /// The tag this chip serializes to.
    let tag: KudzuCore.Tag

    @MainActor
    init(tag: KudzuCore.Tag, label: ChipLabel, font: NSFont) {
        self.tag = tag
        super.init(data: nil, ofType: nil)
        attachmentCell = ChipCell(label: label, font: font)
    }

    required init?(coder: NSCoder) {
        nil
    }

    /// The drawing cell, which carries derived presentation state.
    @MainActor var chipCell: ChipCell? { attachmentCell as? ChipCell }
}

/// Draws a chip as a tinted capsule with an SF Symbol and label, aligned to the text baseline.
final class ChipCell: NSTextAttachmentCell {
    /// What the chip shows; changing it invalidates the cached size.
    var label: ChipLabel {
        didSet {
            guard label != oldValue else { return }
            cachedSize = nil
            cachedSymbol = nil
            updateAccessibility()
        }
    }
    /// Set by editor lint (duplicate cursor, conflicting names, ...).
    var hasLintWarning = false
    /// Editor body font; the chip occupies that font's ascender-to-descender box.
    var bodyFont: NSFont { didSet { cachedSize = nil; cachedSymbol = nil } }

    private var cachedSize: NSSize?
    private var cachedSymbol: NSImage??

    init(label: ChipLabel, font: NSFont) {
        self.label = label
        self.bodyFont = font
        super.init(textCell: "")
        updateAccessibility()
    }

    required init(coder: NSCoder) {
        fatalError("ChipCell is not archived")
    }

    private var isWarning: Bool { label.isWarning || hasLintWarning }
    private var labelFont: NSFont { ChipStyle.labelFont(for: bodyFont) }

    private func updateAccessibility() {
        setAccessibilityLabel(label.accessibilityLabel)
        setAccessibilityRoleDescription("tag")
    }

    // MARK: Layout

    // The cell protocol's layout queries are nonisolated, but TextKit 1 only calls them on the main thread.
    nonisolated override func cellSize() -> NSSize {
        MainActor.assumeIsolated { computeSize() }
    }

    private func computeSize() -> NSSize {
        if let cachedSize { return cachedSize }
        let height = bodyFont.ascender - bodyFont.descender
        let textWidth = ceil(attributedTitle(color: .labelColor, detailColor: .labelColor).size().width)
        let width = ChipStyle.outerMargin * 2 + ChipStyle.horizontalPadding * 2 + iconSize.width + ChipStyle.iconGap + textWidth
        let size = NSSize(width: ceil(width), height: height)
        cachedSize = size
        return size
    }

    /// Spanning exactly descender to ascender (unrounded) keeps the chip inside the line box, so inserting one
    /// never changes line height.
    nonisolated override func cellBaselineOffset() -> NSPoint {
        MainActor.assumeIsolated { NSPoint(x: 0, y: bodyFont.descender) }
    }

    override func wantsToTrackMouse() -> Bool { true }

    // MARK: Drawing

    override func draw(withFrame cellFrame: NSRect, in controlView: NSView?) {
        drawChip(in: cellFrame, controlView: controlView, selected: false)
    }

    override func draw(withFrame cellFrame: NSRect, in controlView: NSView?, characterIndex charIndex: Int,
                       layoutManager: NSLayoutManager) {
        var selected = false
        if let textView = controlView as? NSTextView {
            selected = textView.selectedRanges.contains { NSLocationInRange(charIndex, $0.rangeValue) }
        }
        drawChip(in: cellFrame, controlView: controlView, selected: selected)
    }

    private func drawChip(in frame: NSRect, controlView: NSView?, selected: Bool) {
        let appearance = controlView?.effectiveAppearance ?? NSAppearance.currentDrawing()
        appearance.performAsCurrentDrawingAppearance {
            let isDark = appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
            let isActive = (controlView?.window?.isKeyWindow ?? false)
                && controlView?.window?.firstResponder === controlView
            let tint = ChipStyle.tint(for: label.kind, warning: isWarning).usingColorSpace(.sRGB) ?? .systemBlue
            let capsule = frame.insetBy(dx: ChipStyle.outerMargin, dy: 0.5)
            let path = NSBezierPath(roundedRect: capsule, xRadius: capsule.height / 2, yRadius: capsule.height / 2)

            let foreground: NSColor
            let detailColor: NSColor
            if selected {
                (isActive ? tint : tint.withAlphaComponent(isDark ? 0.55 : 0.45)).setFill()
                path.fill()
                foreground = .white
                detailColor = NSColor.white.withAlphaComponent(0.85)
            } else {
                tint.withAlphaComponent(isDark ? 0.26 : 0.13).setFill()
                path.fill()
                tint.withAlphaComponent(isDark ? 0.45 : 0.32).setStroke()
                path.lineWidth = 0.75
                let inset = NSBezierPath(roundedRect: capsule.insetBy(dx: 0.375, dy: 0.375),
                                         xRadius: (capsule.height - 0.75) / 2, yRadius: (capsule.height - 0.75) / 2)
                inset.lineWidth = 0.75
                inset.stroke()
                foreground = isDark ? tint.blended(withFraction: 0.35, of: .white) ?? tint
                    : tint.blended(withFraction: 0.25, of: .black) ?? tint
                detailColor = foreground.withAlphaComponent(0.75)
            }

            var x = capsule.minX + ChipStyle.horizontalPadding
            if let icon = symbolImage(color: foreground) {
                let rect = NSRect(x: x, y: capsule.midY - iconSize.height / 2, width: iconSize.width, height: iconSize.height)
                icon.draw(in: rect, from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: nil)
            }
            x += iconSize.width + ChipStyle.iconGap
            let title = attributedTitle(color: foreground, detailColor: detailColor)
            let textHeight = labelFont.ascender - labelFont.descender
            // Centering the glyph box works in flipped and unflipped views alike.
            title.draw(at: NSPoint(x: x, y: (capsule.midY - textHeight / 2).rounded()))
        }
    }

    // MARK: Content

    private func attributedTitle(color: NSColor, detailColor: NSColor) -> NSAttributedString {
        let result = NSMutableAttributedString(string: label.title, attributes: [.font: labelFont, .foregroundColor: color])
        if let detail = label.detail {
            result.append(NSAttributedString(string: " · " + detail, attributes: [
                .font: NSFont.systemFont(ofSize: labelFont.pointSize, weight: .regular), .foregroundColor: detailColor,
            ]))
        }
        return result
    }

    private var symbolConfiguration: NSImage.SymbolConfiguration {
        NSImage.SymbolConfiguration(pointSize: labelFont.pointSize - 0.5, weight: .medium, scale: .small)
    }

    /// The untinted symbol at label size, cached because drawing happens on every redraw.
    private var baseSymbol: NSImage? {
        if let cachedSymbol { return cachedSymbol }
        let image = NSImage(systemSymbolName: label.symbolName, accessibilityDescription: nil)?
            .withSymbolConfiguration(symbolConfiguration)
        cachedSymbol = .some(image)
        return image
    }

    private var iconSize: NSSize { baseSymbol?.size ?? NSSize(width: 10, height: 10) }

    private func symbolImage(color: NSColor) -> NSImage? {
        baseSymbol?.withSymbolConfiguration(symbolConfiguration.applying(.init(paletteColors: [color])))
    }
}
