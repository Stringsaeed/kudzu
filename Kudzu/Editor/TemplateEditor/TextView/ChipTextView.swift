import AppKit
import KudzuCore
import KudzuUI

/// Clipboard and paste hooks the chip text view delegates to its coordinator.
@MainActor
protocol ChipTextViewClipboard: AnyObject {
    /// Template source for the given character ranges.
    func sourceText(for ranges: [NSRange]) -> String
    /// Inserts pasted or dropped plain text at the current selection.
    func insertPlainText(_ text: String) -> Bool
}

/// A TextKit 1 text view that copies template source, pastes plain text only and draws a placeholder.
final class ChipTextView: NSTextView {
    /// Receives clipboard requests.
    weak var clipboard: ChipTextViewClipboard?
    /// Shown while the editor is empty.
    var placeholder = "Type text, or / to insert a tag" { didSet { needsDisplay = true } }
    /// Called when the view gains or loses first-responder status; feeds `KzEditorFrame`'s focus edge.
    var onFocusChange: ((Bool) -> Void)?
    /// The index of the chip under the pointer, drawn with the hover layer.
    private(set) var hoveredChipIndex: Int?
    private var trackingArea: NSTrackingArea?

    /// Builds a scrollable chip text view on an explicit TextKit 1 stack, which draws attachment cells reliably.
    static func makeScrollView() -> (NSScrollView, ChipTextView) {
        let storage = NSTextStorage()
        let layoutManager = ChipLayoutManager()
        storage.addLayoutManager(layoutManager)
        let container = NSTextContainer(size: NSSize(width: 0, height: CGFloat.greatestFiniteMagnitude))
        container.widthTracksTextView = true
        layoutManager.addTextContainer(container)

        let textView = ChipTextView(frame: .zero, textContainer: container)
        textView.minSize = .zero
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.autoresizingMask = [.width]
        textView.isRichText = true
        textView.importsGraphics = false
        textView.allowsImageEditing = false
        textView.usesFontPanel = false
        textView.usesRuler = false
        textView.allowsUndo = true
        textView.smartInsertDeleteEnabled = false
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.isAutomaticSpellingCorrectionEnabled = false
        textView.isAutomaticLinkDetectionEnabled = false
        textView.isAutomaticDataDetectionEnabled = false
        textView.isAutomaticTextCompletionEnabled = false
        // Diagnostics reuse the spelling underline, so system spell checking would be ambiguous.
        textView.isContinuousSpellCheckingEnabled = false
        textView.isGrammarCheckingEnabled = false
        textView.setAccessibilityLabel("Snippet content")
        // The `content` style's insets, line metrics, caret and typing attributes; the frame draws the fill.
        KzEditorStyle.configure(textView)
        textView.selectedTextAttributes = [.backgroundColor: KzColorToken.vine.nsColor.withAlphaComponent(0.35)]

        let scrollView = ChipScrollView()
        scrollView.drawsBackground = false
        scrollView.borderType = .noBorder
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.documentView = textView
        return (scrollView, textView)
    }

    // MARK: Copy

    override var writablePasteboardTypes: [NSPasteboard.PasteboardType] { [.string] }

    override func writeSelection(to pboard: NSPasteboard, types: [NSPasteboard.PasteboardType]) -> Bool {
        pboard.declareTypes([.string], owner: nil)
        return writeSelection(to: pboard, type: .string)
    }

    override func writeSelection(to pboard: NSPasteboard, type: NSPasteboard.PasteboardType) -> Bool {
        guard type == .string, let clipboard else { return super.writeSelection(to: pboard, type: type) }
        let ranges = selectedRanges.map(\.rangeValue).filter { $0.length > 0 }
        guard !ranges.isEmpty else { return false }
        return pboard.setString(clipboard.sourceText(for: ranges), forType: .string)
    }

    // MARK: Paste and drop

    override var readablePasteboardTypes: [NSPasteboard.PasteboardType] { [.string] }

    override func readSelection(from pboard: NSPasteboard, type: NSPasteboard.PasteboardType) -> Bool {
        guard let clipboard, let text = pboard.string(forType: .string) else { return false }
        return clipboard.insertPlainText(text)
    }

    override func pasteAsRichText(_ sender: Any?) { paste(sender) }
    override func pasteAsPlainText(_ sender: Any?) { paste(sender) }

    // Formatting is meaningless in plain-text templates; ignore font and style commands.
    override func changeFont(_ sender: Any?) {}
    override func underline(_ sender: Any?) {}
    override func changeAttributes(_ sender: Any?) {}

    // MARK: Undo

    // The window would otherwise route Undo to its own manager; this view's comes from its delegate.
    @objc func undo(_ sender: Any?) { undoManager?.undo() }
    @objc func redo(_ sender: Any?) { undoManager?.redo() }

    override func validateUserInterfaceItem(_ item: any NSValidatedUserInterfaceItem) -> Bool {
        switch item.action {
        case #selector(undo(_:)):
            (item as? NSMenuItem)?.title = undoManager?.undoMenuItemTitle ?? "Undo"
            return undoManager?.canUndo == true
        case #selector(redo(_:)):
            (item as? NSMenuItem)?.title = undoManager?.redoMenuItemTitle ?? "Redo"
            return undoManager?.canRedo == true
        default:
            return super.validateUserInterfaceItem(item)
        }
    }

    // MARK: Focus

    override func becomeFirstResponder() -> Bool {
        let result = super.becomeFirstResponder()
        if result {
            needsDisplay = true
            onFocusChange?(true)
        }
        return result
    }

    override func resignFirstResponder() -> Bool {
        let result = super.resignFirstResponder()
        if result {
            // Selected chips switch between their focused and unfocused look.
            needsDisplay = true
            onFocusChange?(false)
        }
        return result
    }

    // MARK: Hover

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea { removeTrackingArea(trackingArea) }
        let area = NSTrackingArea(rect: .zero, options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
                                  owner: self, userInfo: nil)
        addTrackingArea(area)
        trackingArea = area
    }

    override func mouseMoved(with event: NSEvent) {
        super.mouseMoved(with: event)
        setHoveredChip(chipIndex(at: convert(event.locationInWindow, from: nil)))
    }

    override func mouseExited(with event: NSEvent) {
        super.mouseExited(with: event)
        setHoveredChip(nil)
    }

    /// The index of the chip drawn at `point` (view coordinates), if any.
    func chipIndex(at point: NSPoint) -> Int? {
        guard let layoutManager, let textContainer, let storage = textStorage, storage.length > 0 else { return nil }
        let local = NSPoint(x: point.x - textContainerOrigin.x, y: point.y - textContainerOrigin.y)
        var fraction: CGFloat = 0
        let glyph = layoutManager.glyphIndex(for: local, in: textContainer, fractionOfDistanceThroughGlyph: &fraction)
        guard glyph < layoutManager.numberOfGlyphs else { return nil }
        let rect = layoutManager.boundingRect(forGlyphRange: NSRange(location: glyph, length: 1), in: textContainer)
        guard rect.contains(local) else { return nil }
        let index = layoutManager.characterIndexForGlyph(at: glyph)
        return ChipFactory.chip(in: storage, at: index) == nil ? nil : index
    }

    func setHoveredChip(_ index: Int?) {
        guard index != hoveredChipIndex, let layoutManager else { return }
        for old in [hoveredChipIndex, index].compactMap({ $0 }) where old < (textStorage?.length ?? 0) {
            layoutManager.invalidateDisplay(forCharacterRange: NSRange(location: old, length: 1))
        }
        hoveredChipIndex = index
    }

    // MARK: Placeholder

    override func didChangeText() {
        super.didChangeText()
        // Edits shift or remove chips, so a stale hover index could point at the wrong one.
        hoveredChipIndex = nil
        needsDisplay = true
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        guard textStorage?.length == 0, !placeholder.isEmpty else { return }
        let origin = NSPoint(x: textContainerOrigin.x + KzEditorStyle.lineFragmentPadding, y: textContainerOrigin.y)
        let width = max(0, bounds.width - origin.x - KzEditorStyle.containerInset.width)
        let rect = NSRect(x: origin.x, y: origin.y, width: width, height: KzEditorStyle.textStyle.spec.lineHeight)
        NSAttributedString(string: placeholder, attributes: KzEditorStyle.contentAttributes(color: KzColorToken.inkMuted.nsColor))
            .draw(in: rect)
    }
}

/// Keeps the text view at least as tall as the visible area, so a click below the last line still lands in it.
final class ChipScrollView: NSScrollView {
    override func tile() {
        super.tile()
        guard let textView = documentView as? NSTextView, textView.minSize.height != contentSize.height else { return }
        textView.minSize = NSSize(width: 0, height: contentSize.height)
        textView.sizeToFit()
    }
}

/// A diagnostic underline: a range of editor content and how serious it is.
struct ChipSquiggle: Equatable {
    var range: NSRange
    var severity: ChipSeverity
}

/// Skips the rectangular selection fill behind a lone selected chip, which draws its own selected look,
/// and draws diagnostic squiggles in `danger` / `warning` (the system spelling underline can't be recoloured).
final class ChipLayoutManager: NSLayoutManager {
    private(set) var squiggles: [ChipSquiggle] = []

    /// Replaces the squiggles and redraws only the ranges that changed.
    func setSquiggles(_ new: [ChipSquiggle]) {
        guard new != squiggles else { return }
        let length = textStorage?.length ?? 0
        for squiggle in squiggles + new {
            let range = NSIntersectionRange(squiggle.range, NSRange(location: 0, length: length))
            if range.length > 0 { invalidateDisplay(forCharacterRange: range) }
        }
        squiggles = new
    }

    override func fillBackgroundRectArray(_ rectArray: UnsafePointer<NSRect>, count rectCount: Int,
                                          forCharacterRange charRange: NSRange, color: NSColor) {
        if charRange.length == 1, textStorage?.attribute(.attachment, at: charRange.location, effectiveRange: nil) is ChipAttachment {
            return
        }
        super.fillBackgroundRectArray(rectArray, count: rectCount, forCharacterRange: charRange, color: color)
    }

    override func drawGlyphs(forGlyphRange glyphsToShow: NSRange, at origin: NSPoint) {
        super.drawGlyphs(forGlyphRange: glyphsToShow, at: origin)
        let length = textStorage?.length ?? 0
        for squiggle in squiggles {
            let characters = NSIntersectionRange(squiggle.range, NSRange(location: 0, length: length))
            guard characters.length > 0 else { continue }
            let visible = NSIntersectionRange(glyphRange(forCharacterRange: characters, actualCharacterRange: nil), glyphsToShow)
            guard visible.length > 0 else { continue }
            enumerateLineFragments(forGlyphRange: visible) { rect, _, container, lineGlyphs, _ in
                let part = NSIntersectionRange(lineGlyphs, visible)
                guard part.length > 0 else { return }
                let bounds = self.boundingRect(forGlyphRange: part, in: container)
                // A little under the baseline, where a text underline would sit.
                let y = origin.y + rect.minY + self.location(forGlyphAt: part.location).y + 3
                Self.drawWave(from: origin.x + bounds.minX, to: origin.x + bounds.maxX, y: y, color: squiggle.severity.squiggle)
            }
        }
    }

    private static func drawWave(from start: CGFloat, to end: CGFloat, y: CGFloat, color: NSColor) {
        guard end > start else { return }
        let step: CGFloat = 2, amplitude: CGFloat = 1.25
        let path = NSBezierPath()
        path.lineWidth = 1
        path.lineJoinStyle = .round
        path.lineCapStyle = .round
        path.move(to: NSPoint(x: start, y: y))
        var x = start, up = true
        while x < end {
            x = min(x + step, end)
            path.line(to: NSPoint(x: x, y: y + (up ? -amplitude : amplitude)))
            up.toggle()
        }
        color.setStroke()
        path.stroke()
    }
}
