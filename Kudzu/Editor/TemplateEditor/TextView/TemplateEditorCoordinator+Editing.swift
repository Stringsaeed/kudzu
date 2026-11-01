import AppKit
import KudzuCore

/// Undoable edits: chip insertion, chip attribute changes, typed-tag conversion and paste.
extension TemplateEditorCoordinator {
    /// Replaces `range` through the text view so the change is undoable and reported like typing.
    @discardableResult
    func replaceCharacters(in range: NSRange, with content: NSAttributedString, select selection: NSRange? = nil) -> Bool {
        guard let textView, let storage, range.location != NSNotFound, NSMaxRange(range) <= storage.length,
              textView.shouldChangeText(in: range, replacementString: content.string) else { return false }
        storage.replaceCharacters(in: range, with: content)
        textView.didChangeText()
        textView.setSelectedRange(selection ?? NSRange(location: range.location + content.length, length: 0))
        textView.scrollRangeToVisible(textView.selectedRange())
        return true
    }

    /// Inserts an item's chip in place of `range`, opening its popover when it needs configuring.
    func insert(_ item: TemplateInsertItem, replacing range: NSRange) {
        guard let textView else { return }
        let tag = item.makeTag(existing: segments())
        textView.breakUndoCoalescing()
        guard replaceCharacters(in: range, with: factory.chip(tag)) else { return }
        undoManager.setActionName("Insert \(item.title.replacingOccurrences(of: "…", with: ""))")
        if item.editsOnInsert {
            textView.setSelectedRange(NSRange(location: range.location, length: 1))
            popovers.show(chipAt: range.location)
        }
    }

    /// Inserts from the "+" menu at the current selection.
    func insertFromMenu(_ item: TemplateInsertItem) {
        guard let textView else { return }
        textView.window?.makeFirstResponder(textView)
        closeSlashMenu()
        insert(item, replacing: textView.selectedRange())
    }

    /// Replaces the chip at `index`. Popover sessions coalesce: only their first change registers undo,
    /// because NSTextView's undo record captures the original chip and re-reads the latest one for redo.
    func updateChip(at index: Int, to tag: KudzuCore.Tag, coalescing: Bool) {
        guard let textView, let storage, let current = ChipFactory.chip(in: storage, at: index), current.tag != tag else { return }
        if coalescing {
            undoManager.disableUndoRegistration()
        } else {
            textView.breakUndoCoalescing()
        }
        replaceCharacters(in: NSRange(location: index, length: 1), with: factory.chip(tag),
                          select: NSRange(location: index, length: 1))
        if coalescing {
            undoManager.enableUndoRegistration()
        } else {
            undoManager.setActionName("Edit Tag")
        }
    }

    /// Turns a just-typed `{{tag …}}` into a chip. Returns true if it replaced text.
    func convertTypedTag(endingAt caret: Int) -> Bool {
        guard let storage, caret <= storage.length else { return false }
        let string = storage.string as NSString
        var start = caret
        // Only the literal run on this line can hold the tag; cap the scan for very long lines.
        while start > 0, caret - start < 2_000 {
            let unit = string.character(at: start - 1)
            if unit == 0xFFFC || unit == 0x0A { break }
            start -= 1
        }
        let run = string.substring(with: NSRange(location: start, length: caret - start))
        guard let found = TemplateSegments.typedTag(in: run, endingAt: run.utf16.count) else { return false }
        textView?.breakUndoCoalescing()
        let range = NSRange(location: start + found.range.lowerBound, length: found.range.count)
        guard replaceCharacters(in: range, with: factory.chip(found.tag)) else { return false }
        undoManager.setActionName("Insert Tag")
        return true
    }

    /// The selected chip's index when exactly one chip is selected.
    var selectedChipIndex: Int? {
        guard let textView, let storage else { return nil }
        let range = textView.selectedRange()
        guard range.length == 1, ChipFactory.chip(in: storage, at: range.location) != nil else { return nil }
        return range.location
    }

    /// The chip's rectangle in text view coordinates.
    func rect(forChipAt index: Int) -> NSRect {
        guard let textView, let layoutManager = textView.layoutManager, let container = textView.textContainer else { return .zero }
        let glyphs = layoutManager.glyphRange(forCharacterRange: NSRange(location: index, length: 1), actualCharacterRange: nil)
        layoutManager.ensureLayout(forGlyphRange: glyphs)
        var rect = layoutManager.boundingRect(forGlyphRange: glyphs, in: container)
        rect.origin.x += textView.textContainerOrigin.x
        rect.origin.y += textView.textContainerOrigin.y
        return rect
    }

    /// What a chip popover needs besides the tag.
    func inspectorContext(forChipAt index: Int) -> ChipInspectorContext {
        let segments = segments()
        var offset = 0
        var segmentIndex: Int?
        for (position, segment) in segments.enumerated() {
            if offset == index, case .chip = segment { segmentIndex = position; break }
            offset += segment.displayLength
            if offset > index { break }
        }
        return ChipInspectorContext(
            snippets: configuration.snippets,
            currentSnippetID: configuration.currentSnippetID,
            nameConflicts: { name, kind in
                TemplateEditorLint.nameConflicts(name, kind: kind, in: segments, excludingChipAt: segmentIndex)
            })
    }

    /// Restores focus after a popover closes.
    func popoverDidClose(chipAt index: Int?) {
        guard let textView else { return }
        textView.window?.makeFirstResponder(textView)
        if let index, let storage, ChipFactory.chip(in: storage, at: index) != nil {
            textView.setSelectedRange(NSRange(location: index, length: 1))
        }
    }
}

// MARK: - Clipboard

extension TemplateEditorCoordinator: ChipTextViewClipboard {
    /// Template source when the selection holds chips; the literal text otherwise, so ordinary copying stays lossless.
    func sourceText(for ranges: [NSRange]) -> String {
        guard let storage else { return "" }
        let segments = ranges.flatMap { ChipFactory.segments(in: storage, range: $0) }
        guard segments.contains(where: { if case .chip = $0 { true } else { false } }) else {
            return segments.map { if case .text(let text) = $0 { text } else { "" } }.joined()
        }
        return TemplateSegments.serialize(segments)
    }

    func insertPlainText(_ text: String) -> Bool {
        guard let textView else { return false }
        let range = textView.rangeForUserTextChange
        guard range.location != NSNotFound else { return false }
        textView.breakUndoCoalescing()
        let content = factory.attributedString(for: TemplateSegments.pasted(text))
        guard replaceCharacters(in: range, with: content) else { return false }
        undoManager.setActionName("Paste")
        return true
    }
}
