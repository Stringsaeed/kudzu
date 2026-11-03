import AppKit
import KudzuCore

extension TemplateEditorCoordinator: NSTextViewDelegate {
    func undoManager(for view: NSTextView) -> UndoManager? { undoManager }

    func textView(_ textView: NSTextView, shouldChangeTextIn affectedCharRange: NSRange, replacementString: String?) -> Bool {
        pendingEdit = replacementString.map { (affectedCharRange, $0) }
        return true
    }

    func textView(_ textView: NSTextView, shouldChangeTypingAttributes oldTypingAttributes: [String: Any],
                  toAttributes newTypingAttributes: [NSAttributedString.Key: Any]) -> [NSAttributedString.Key: Any] {
        // Never inherit an attachment or stray formatting from a neighboring chip.
        factory.textAttributes
    }

    func textDidChange(_ notification: Notification) {
        guard let storage else { return }
        let edit = pendingEdit
        pendingEdit = nil
        // Converting re-enters this method through `didChangeText`, which performs the sync.
        if let edit, edit.text == "}", convertTypedTag(endingAt: edit.range.location + 1) { return }
        let segments = ChipFactory.segments(in: storage)
        syncSource(segments)
        refreshDecorations(segments)
        if let edit, edit.text == "/", SlashCommand.canOpen(in: storage.string, slashAt: edit.range.location) {
            slashAnchor = edit.range.location
        }
        updateSlashMenu()
    }

    func textViewDidChangeSelection(_ notification: Notification) {
        updateSlashMenu()
    }

    func textView(_ textView: NSTextView, clickedOn cell: any NSTextAttachmentCellProtocol, in cellFrame: NSRect, at charIndex: Int) {
        guard let storage, ChipFactory.chip(in: storage, at: charIndex) != nil else { return }
        closeSlashMenu()
        textView.setSelectedRange(NSRange(location: charIndex, length: 1))
        popovers.show(chipAt: charIndex)
    }

    func textView(_ textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        guard !textView.hasMarkedText() else { return false }
        if slashAnchor != nil {
            switch commandSelector {
            case #selector(NSResponder.cancelOperation(_:)), #selector(NSTextView.complete(_:)):
                closeSlashMenu()
                return true
            case #selector(NSResponder.moveUp(_:)) where slashMenu.isVisible:
                slashMenu.moveSelection(by: -1)
                return true
            case #selector(NSResponder.moveDown(_:)) where slashMenu.isVisible:
                slashMenu.moveSelection(by: 1)
                return true
            case #selector(NSResponder.insertNewline(_:)) where slashMenu.isVisible,
                 #selector(NSResponder.insertTab(_:)) where slashMenu.isVisible:
                if let item = slashMenu.selectedItem { insertFromSlash(item) }
                return true
            default:
                break
            }
        }
        if commandSelector == #selector(NSResponder.insertNewline(_:)), let index = selectedChipIndex {
            popovers.show(chipAt: index)
            return true
        }
        if commandSelector == #selector(NSResponder.cancelOperation(_:)) {
            if popovers.isOpen {
                popovers.close()
                return true
            }
            textView.window?.makeFirstResponder(nil)
            return true
        }
        return false
    }
}

// MARK: - Slash menu

extension TemplateEditorCoordinator {
    /// Shows, filters, moves or closes the `/` menu to match the text after the anchor.
    func updateSlashMenu() {
        guard let anchor = slashAnchor else { return }
        guard let textView, let storage, let window = textView.window else { closeSlashMenu(); return }
        let selection = textView.selectedRange()
        guard selection.length == 0,
              let query = SlashCommand.query(in: storage.string, slashAt: anchor, caret: selection.location) else {
            closeSlashMenu()
            return
        }
        let items = TemplateInsertItem.matching(query)
        guard !items.isEmpty else {
            // Keep the session so deleting back to a matching query reopens the menu.
            slashMenu.hide()
            return
        }
        let caret = textView.firstRect(forCharacterRange: NSRange(location: anchor, length: 1), actualRange: nil)
        slashMenu.show(items: items, query: query, below: caret, in: window)
    }

    func closeSlashMenu() {
        slashAnchor = nil
        slashMenu.hide()
    }

    /// Replaces the typed `/query` with the picked item's chip.
    func insertFromSlash(_ item: TemplateInsertItem) {
        guard let anchor = slashAnchor, let textView else { return }
        let caret = textView.selectedRange().location
        closeSlashMenu()
        guard caret > anchor else { return }
        insert(item, replacing: NSRange(location: anchor, length: caret - anchor))
    }
}
