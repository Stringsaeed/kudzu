import AppKit
import KudzuCore
import KudzuUI

/// A text attachment that stands for exactly one template tag.
///
/// Attachments are immutable with respect to their tag: edits replace the attachment, so
/// `NSTextView`'s undo stack (which stores attributed substrings) restores the old tag.
final class ChipAttachment: NSTextAttachment {
    /// The tag this chip serializes to.
    let tag: KudzuCore.Tag

    @MainActor
    init(tag: KudzuCore.Tag, label: ChipLabel) {
        self.tag = tag
        super.init(data: nil, ofType: nil)
        attachmentCell = ChipCell(label: label)
    }

    required init?(coder: NSCoder) {
        nil
    }

    /// The drawing cell, which carries derived presentation state.
    @MainActor var chipCell: ChipCell? { attachmentCell as? ChipCell }
}

/// Draws a chip as the design system's tag: `code` mono in `vine-ink` on `vine-soft` (`radius-xs`), 20pt tall,
/// raised off the baseline by `KzEditorTag`'s metrics. Problems recolour it (`warning` or `danger` on their soft fills).
final class ChipCell: NSTextAttachmentCell {
    /// What the chip shows; changing it invalidates the cached size.
    var label: ChipLabel {
        didSet {
            guard label != oldValue else { return }
            cachedSize = nil
            updateAccessibility()
        }
    }
    /// Set by editor lint (duplicate cursor, conflicting names, ...).
    var lintSeverity: ChipSeverity?

    private var cachedSize: NSSize?

    init(label: ChipLabel) {
        self.label = label
        super.init(textCell: "")
        updateAccessibility()
    }

    required init(coder: NSCoder) {
        fatalError("ChipCell is not archived")
    }

    /// The chip's problem, if any. Unrecognized tags are errors, shown as written in `danger`, never the accent.
    var severity: ChipSeverity? {
        if label.kind == .unknown { return .error }
        if let lintSeverity { return lintSeverity }
        return label.isWarning ? .warning : nil
    }

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
        let size = KzEditorTag.cellSize(forTitleWidth: ceil(attributedTitle(color: .labelColor).size().width))
        let rounded = NSSize(width: ceil(size.width), height: size.height)
        cachedSize = rounded
        return rounded
    }

    nonisolated override func cellBaselineOffset() -> NSPoint {
        MainActor.assumeIsolated { NSPoint(x: 0, y: KzEditorTag.baselineOffset) }
    }

    override func wantsToTrackMouse() -> Bool { true }

    // MARK: Drawing

    override func draw(withFrame cellFrame: NSRect, in controlView: NSView?) {
        drawChip(in: cellFrame, controlView: controlView, selected: false, hovered: false)
    }

    override func draw(withFrame cellFrame: NSRect, in controlView: NSView?, characterIndex charIndex: Int,
                       layoutManager: NSLayoutManager) {
        var selected = false
        var hovered = false
        if let textView = controlView as? NSTextView {
            selected = textView.selectedRanges.contains { NSLocationInRange(charIndex, $0.rangeValue) }
            hovered = (textView as? ChipTextView)?.hoveredChipIndex == charIndex
        }
        drawChip(in: cellFrame, controlView: controlView, selected: selected, hovered: hovered)
    }

    /// States: rest, hover (a tint of the chip's ink), selected in the focused editor (solid fill, like
    /// a selected row's emphasis) and selected while the editor is not focused (the soft fill with a 1pt outline).
    private func drawChip(in frame: NSRect, controlView: NSView?, selected: Bool, hovered: Bool) {
        let appearance = controlView?.effectiveAppearance ?? NSAppearance.currentDrawing()
        appearance.performAsCurrentDrawingAppearance {
            let palette = ChipPalette.palette(for: severity)
            let isActive = (controlView?.window?.isKeyWindow ?? false) && controlView?.window?.firstResponder === controlView
            let box = NSRect(x: frame.minX + KzEditorTag.horizontalMargin, y: frame.midY - KzEditorTag.height / 2,
                             width: frame.width - 2 * KzEditorTag.horizontalMargin, height: KzEditorTag.height)
            let radius = KzEditorTag.cornerRadius
            let path = NSBezierPath(roundedRect: box, xRadius: radius, yRadius: radius)

            let solid = selected && isActive
            (solid ? palette.selectedFill : palette.fill).setFill()
            path.fill()
            if hovered, !solid {
                // Tinted with the chip's own ink: the shared `surface-hover` layer is too faint on a soft fill.
                palette.ink.withAlphaComponent(0.14).setFill()
                path.fill()
            }
            if selected, !isActive {
                let outline = NSBezierPath(roundedRect: box.insetBy(dx: 0.5, dy: 0.5), xRadius: radius - 0.5, yRadius: radius - 0.5)
                outline.lineWidth = 1
                palette.ink.setStroke()
                outline.stroke()
            }

            let title = attributedTitle(color: solid ? palette.selectedInk : palette.ink, detailAlpha: solid ? 0.85 : 0.72)
            let textHeight = KzEditorTag.font.ascender - KzEditorTag.font.descender
            // Centring the glyph box matches CSS half-leading, in flipped and unflipped views alike.
            title.draw(at: NSPoint(x: box.minX + KzEditorTag.horizontalPadding, y: box.midY - textHeight / 2))
        }
    }

    // MARK: Content

    /// The title in the tag font, with the detail after a middle dot a little quieter.
    private func attributedTitle(color: NSColor, detailAlpha: CGFloat = 0.72) -> NSAttributedString {
        let font = KzEditorTag.font
        let result = NSMutableAttributedString(string: label.title, attributes: [.font: font, .foregroundColor: color])
        if let detail = label.detail {
            result.append(NSAttributedString(string: " · " + detail, attributes: [
                .font: font, .foregroundColor: color.withAlphaComponent(detailAlpha),
            ]))
        }
        return result
    }
}
