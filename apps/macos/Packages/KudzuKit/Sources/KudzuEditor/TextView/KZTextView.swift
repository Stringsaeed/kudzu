// The editor's NSTextView. Everything that is native stays native (caret, selection, IME, spell
// checking, services, drag and drop); this subclass only adds what the template editor needs:
// source-text copy/paste, the editor's own undo stack, the chip hover ring and click, ⌘B/⌘I/⌘U,
// Escape, the 2 pt iris caret and the placeholder.

import AppKit
import KudzuCore
import KudzuDesign

/// What kind of user input produced the current edit (for undo coalescing and the slash menu).
enum EditorInputKind { case typing, deleting, other }

@MainActor
public final class KZTextView: NSTextView {
    weak var controller: TemplateTextController?
    var inputKind: EditorInputKind = .other
    /// Storage index of the chip under the mouse (its ring is drawn at 70 %).
    private(set) var hoveredAtomIndex: Int?
    var placeholderString = "" { didSet { needsDisplay = true } }
    var placeholderFont = NSFont.systemFont(ofSize: 13)
    private var hoverArea: NSTrackingArea?

    // MARK: Undo

    /// The editor keeps its own history (text + formatting + selection) and handles `undo:` /
    /// `redo:` below; NSTextView's built-in registration is kept out of it entirely.
    public override var undoManager: UndoManager? { nil }

    @objc func undo(_ sender: Any?) { controller?.undoManager.undo() }
    @objc func redo(_ sender: Any?) { controller?.undoManager.redo() }

    public override func validateUserInterfaceItem(_ item: any NSValidatedUserInterfaceItem) -> Bool {
        switch item.action {
        case #selector(undo(_:)):
            if let menu = item as? NSMenuItem, let manager = controller?.undoManager { menu.title = manager.undoMenuItemTitle }
            return controller?.undoManager.canUndo ?? false
        case #selector(redo(_:)):
            if let menu = item as? NSMenuItem, let manager = controller?.undoManager { menu.title = manager.redoMenuItemTitle }
            return controller?.undoManager.canRedo ?? false
        default:
            return super.validateUserInterfaceItem(item)
        }
    }

    // MARK: Focus

    public override func becomeFirstResponder() -> Bool {
        let became = super.becomeFirstResponder()
        if became { controller?.focusChanged(true) }
        return became
    }

    public override func resignFirstResponder() -> Bool {
        let resigned = super.resignFirstResponder()
        if resigned { controller?.focusChanged(false) }
        return resigned
    }

    // MARK: Input

    public override func insertText(_ string: Any, replacementRange: NSRange) {
        inputKind = .typing
        defer { inputKind = .other }
        super.insertText(string, replacementRange: replacementRange)
    }

    public override func deleteBackward(_ sender: Any?) {
        inputKind = .deleting
        defer { inputKind = .other }
        super.deleteBackward(sender)
    }

    public override func deleteForward(_ sender: Any?) {
        inputKind = .deleting
        defer { inputKind = .other }
        super.deleteForward(sender)
    }

    public override func cancelOperation(_ sender: Any?) {
        if let escape = controller?.onEscape { escape() } else { super.cancelOperation(sender) }
    }

    /// ⌘B, ⌘I and ⌘U toggle a style on the selection in formatted mode (the TS keymap); anything
    /// else falls through to the menus.
    public override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard window?.firstResponder === self,
              event.modifierFlags.intersection(.deviceIndependentFlagsMask).subtracting([.capsLock, .numericPad, .function]) == .command,
              let key = event.charactersIgnoringModifiers?.lowercased() else { return super.performKeyEquivalent(with: event) }
        let style: FormatStyle? = switch key {
        case "b": .bold
        case "i": .italic
        case "u": .underline
        default: nil
        }
        if let style, controller?.changeFormat(style) == true { return true }
        return super.performKeyEquivalent(with: event)
    }

    /// Format ▸ Underline (and any menu item sending `underline:`).
    public override func underline(_ sender: Any?) {
        _ = controller?.changeFormat(.underline)
    }

    // MARK: Pasteboard: the source text, so chips copy as their tags

    public override var writablePasteboardTypes: [NSPasteboard.PasteboardType] { [.string] }
    public override var readablePasteboardTypes: [NSPasteboard.PasteboardType] { [.string] }

    public override func writeSelection(to pboard: NSPasteboard, types: [NSPasteboard.PasteboardType]) -> Bool {
        guard let controller else { return super.writeSelection(to: pboard, types: types) }
        let text = controller.sourceText(ofStorageRanges: selectedRanges.map(\.rangeValue))
        pboard.declareTypes([.string], owner: nil)
        return pboard.setString(text, forType: .string)
    }

    // MARK: Chips: hover and click

    public override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let hoverArea { removeTrackingArea(hoverArea) }
        let area = NSTrackingArea(rect: .zero, options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect], owner: self)
        addTrackingArea(area)
        hoverArea = area
    }

    public override func mouseMoved(with event: NSEvent) {
        super.mouseMoved(with: event)
        let hit = controller?.atomHit(at: convert(event.locationInWindow, from: nil))
        setHovered(hit?.atom.tag != nil ? hit?.index : nil)
        if hit?.atom.tag != nil { NSCursor.pointingHand.set() }
    }

    public override func mouseExited(with event: NSEvent) {
        super.mouseExited(with: event)
        setHovered(nil)
    }

    func setHovered(_ index: Int?) {
        guard index != hoveredAtomIndex else { return }
        let old = hoveredAtomIndex
        hoveredAtomIndex = index
        for i in [old, index].compactMap({ $0 }) where i < textStorage?.length ?? 0 {
            layoutManager?.invalidateDisplay(forCharacterRange: NSRange(location: i, length: 1))
        }
    }

    /// A chip click opens its configuration (the caret stays put); an image click selects it.
    public override func mouseDown(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        if event.clickCount == 1, !event.modifierFlags.contains(.shift), let controller, let hit = controller.atomHit(at: point) {
            switch hit.atom.kind {
            case .chip(let tag):
                controller.chipClicked(tag, rect: hit.rect)
                return
            case .image:
                window?.makeFirstResponder(self)
                setSelectedRange(NSRange(location: hit.index, length: 1))
                return
            }
        }
        super.mouseDown(with: event)
    }

    // MARK: Drawing

    /// A 2 pt iris caret (`border-left: 2px solid var(--iris)`).
    public override func drawInsertionPoint(in rect: NSRect, color: NSColor, turnedOn flag: Bool) {
        var caret = rect
        caret.size.width = 2
        super.drawInsertionPoint(in: caret, color: color, turnedOn: flag)
    }

    public override func setNeedsDisplay(_ invalidRect: NSRect, avoidAdditionalLayout flag: Bool) {
        var rect = invalidRect
        rect.size.width += 2
        super.setNeedsDisplay(rect, avoidAdditionalLayout: flag)
    }

    /// The placeholder: faint, sans, at the first line's text position.
    public override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        guard string.isEmpty, !placeholderString.isEmpty, let controller else { return }
        let origin = textContainerOrigin
        let font = placeholderFont
        let line = controller.lineHeight
        let top = origin.y + max(0, (line - (font.ascender - font.descender)) / 2)
        let width = bounds.width - origin.x * 2
        (placeholderString as NSString).draw(
            with: NSRect(x: origin.x + (textContainer?.lineFragmentPadding ?? 0), y: top, width: max(0, width), height: line * 4),
            options: [.usesLineFragmentOrigin],
            attributes: [.font: font, .foregroundColor: NSColor.kz.faint]
        )
    }
}
