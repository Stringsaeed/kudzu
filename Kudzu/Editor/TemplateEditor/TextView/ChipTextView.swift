import AppKit
import KudzuCore

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
        textView.textContainerInset = NSSize(width: 6, height: 8)
        textView.drawsBackground = false
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

        let scrollView = NSScrollView()
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

    // MARK: Placeholder

    override func didChangeText() {
        super.didChangeText()
        needsDisplay = true
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        guard textStorage?.length == 0, !placeholder.isEmpty else { return }
        let padding = textContainer?.lineFragmentPadding ?? 0
        let origin = NSPoint(x: textContainerOrigin.x + padding, y: textContainerOrigin.y)
        NSAttributedString(string: placeholder, attributes: [
            .font: font ?? NSFont.preferredFont(forTextStyle: .body), .foregroundColor: NSColor.placeholderTextColor,
        ]).draw(at: origin)
    }
}

/// Skips the rectangular selection fill behind a lone selected chip, which draws its own selected capsule.
final class ChipLayoutManager: NSLayoutManager {
    override func fillBackgroundRectArray(_ rectArray: UnsafePointer<NSRect>, count rectCount: Int,
                                          forCharacterRange charRange: NSRange, color: NSColor) {
        if charRange.length == 1, textStorage?.attribute(.attachment, at: charRange.location, effectiveRange: nil) is ChipAttachment {
            return
        }
        super.fillBackgroundRectArray(rectArray, count: rectCount, forCharacterRange: charRange, color: color)
    }
}
