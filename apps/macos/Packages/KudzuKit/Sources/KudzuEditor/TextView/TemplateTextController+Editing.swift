// The edit pipeline. A storage edit (typing, paste, IME, drag, spelling) is converted into a
// source edit while the storage is processing it; when the text view finishes the change, the
// controller reconciles: re-tokenizes, rebuilds chips that appeared or vanished, restyles, records
// undo (coalescing runs of typing and deleting like NSTextView) and publishes the new source.

import AppKit
import KudzuCore

// MARK: - Storage edits → source edits

extension TemplateTextController: NSTextStorageDelegate {
    public nonisolated func textStorage(_ textStorage: NSTextStorage, didProcessEditing editedMask: NSTextStorageEditActions, range editedRange: NSRange, changeInLength delta: Int) {
        MainActor.assumeIsolated {
            guard applying == 0, editedMask.contains(.editedCharacters) else { return }
            recordStorageEdit(editedRange)
        }
    }

    /// The storage outside `editedRange` is unchanged, so the source edit is: the serialized
    /// prefix and suffix stay, the edited characters serialize to the inserted text.
    func recordStorageEdit(_ editedRange: NSRange) {
        let length = storage.length
        let oldLength = live.source.utf16.count
        let prefix = serialize(NSRange(location: 0, length: editedRange.location)).utf16.count
        let suffix = serialize(NSRange(location: editedRange.upperBound, length: length - editedRange.upperBound)).utf16.count
        let inserted = serialize(editedRange)
        let edit = FormattingEdit(range: prefix..<max(prefix, oldLength - suffix), insert: inserted)
        live = live.applying(edit)
        pending.append((edit, textView.inputKind))
    }
}

// MARK: - Text view delegate

extension TemplateTextController: NSTextViewDelegate {
    public func textView(_ textView: NSTextView, shouldChangeTextInRanges affectedRanges: [NSValue], replacementStrings: [String]?) -> Bool {
        // Preserve exact edits: the storage delegate's dirty range includes attribute fixing and
        // the space between disjoint replacements. Mapping that hull would expand nearby marks.
        if applying == 0, pending.isEmpty, !affectedRanges.isEmpty,
           let strings = replacementStrings, strings.count == affectedRanges.count {
            let edits = zip(affectedRanges, strings).map { value, string in
                let range = value.rangeValue
                return FormattingEdit(range: map.sourceRange(range.location..<range.upperBound), insert: string)
            }
            nativeBatch = (live, edits, textView === self.textView ? self.textView.inputKind : .other)
        }
        return true
    }

    public func textDidChange(_ notification: Notification) {
        reconcile()
    }

    public func textViewDidChangeSelection(_ notification: Notification) {
        // After SwiftUI has grown the editor for this edit, so the caret's line exists in the pane.
        if growsFrom != nil { DispatchQueue.main.async { [weak self] in self?.revealCaret() } }
        guard applying == 0 else { return }
        if pending.isEmpty {
            let selection = map.sourceRange(textView.selectedRange().location..<textView.selectedRange().upperBound)
            guard selection != state.selection else { return }
            // A caret move ends a typing run, like NSTextView's own undo coalescing.
            lastUndo = nil
            state.selection = selection
            live.selection = selection
            onSelectionChange?(selection)
            onStateChange?()
        }
    }

    public func textView(_ textView: NSTextView, shouldChangeTypingAttributes oldTypingAttributes: [String: Any] = [:], toAttributes newTypingAttributes: [NSAttributedString.Key: Any] = [:]) -> [NSAttributedString.Key: Any] {
        // Styles come from spans, never from the character before the caret.
        baseAttributes
    }
}

// MARK: - Reconcile

extension TemplateTextController {
    /// Applies the pending user edits: chips, styles, undo, bindings, slash menu.
    func reconcile() {
        guard !pending.isEmpty, !isComposing else { return }
        let scanned = scan().source
        if let batch = nativeBatch {
            nativeBatch = nil
            let expected = batch.before.applying(batch.edits)
            if expected.source.utf16.elementsEqual(scanned.utf16) {
                live = expected
                pending = batch.edits.map { ($0, batch.kind) }
            }
        }
        let edits = pending
        pending = []
        var next = live
        if !scanned.utf16.elementsEqual(next.source.utf16) {
            // Never expected; the storage is the truth for what the user sees.
            next = EditorState(source: scanned, spans: next.spans)
        }
        next.selection = sourceSelection()
        let previous = state
        let single = edits.count == 1 ? edits[0] : nil
        let kind = single?.kind ?? .other
        if !coalesces(kind: kind, edit: single?.edit) {
            registerUndo(restoring: previous, name: kind == .typing ? "Typing" : kind == .deleting ? "Delete" : "Change")
        }
        lastUndo = single.map { (kind, $0.edit) }
        if kind == .other { lastUndo = nil }
        render(next)
        publish()
        if let single, kind == .typing { openSlashMenuIfNeeded(single.edit) }
    }

    /// Continues the undo group of the previous typing (or deleting) edit when it picks up where
    /// that one ended.
    func coalesces(kind: EditorInputKind, edit: FormattingEdit?) -> Bool {
        guard kind != .other, let edit, let last = lastUndo, last.kind == kind else { return false }
        switch kind {
        case .typing:
            return edit.range.isEmpty && edit.range.lowerBound == last.edit.range.lowerBound + last.edit.insert.utf16.count
        case .deleting:
            return edit.insert.isEmpty && (edit.range.upperBound == last.edit.range.lowerBound || edit.range.lowerBound == last.edit.range.lowerBound)
        case .other:
            return false
        }
    }

    /// A `/` typed at the start of a word opens the slash menu at its position.
    func openSlashMenuIfNeeded(_ edit: FormattingEdit) {
        guard edit.insert == "/", let onSlashOpen else { return }
        let position = edit.range.lowerBound
        let text = state.source as NSString
        if position > 0 {
            // ECMAScript `\s`, including BOM but excluding NEL (Foundation differs on both).
            switch text.character(at: position - 1) {
            case 0x09...0x0D, 0x20, 0xA0, 0x1680, 0x2000...0x200A, 0x2028, 0x2029,
                 0x202F, 0x205F, 0x3000, 0xFEFF, 0x28, 0x5B, 0x22, 0x27: break
            default: return
            }
        }
        guard let rect = coordsAt(position) else { return }
        onSlashOpen(position, rect)
    }

    func publish() {
        onChange?(state.source, state.spans)
        onSelectionChange?(state.selection)
        onStateChange?()
    }
}

// MARK: - Undo

extension TemplateTextController {
    func registerUndo(restoring previous: EditorState, name: String) {
        undoManager.beginUndoGrouping()
        undoManager.registerUndo(withTarget: self) { target in
            MainActor.assumeIsolated { target.restore(previous, name: name) }
        }
        undoManager.setActionName(name)
        undoManager.endUndoGrouping()
    }

    /// Undo/redo: restores a snapshot and registers the inverse.
    func restore(_ snapshot: EditorState, name: String) {
        flushPending()
        let current = state
        undoManager.registerUndo(withTarget: self) { target in
            MainActor.assumeIsolated { target.restore(current, name: name) }
        }
        undoManager.setActionName(name)
        lastUndo = nil
        render(snapshot)
        publish()
    }

    /// Reconciles edits still waiting for the text view (an operation arriving mid-change).
    func flushPending() {
        if !pending.isEmpty, !isComposing { reconcile() }
    }
}

// MARK: - Operations (handle, toolbar, keyboard)

extension TemplateTextController {
    /// Applies a model change as one undo step (`isolateHistory: "full"`).
    func perform(_ next: EditorState, name: String) {
        flushPending()
        var previous = state
        previous.selection = currentState.selection
        registerUndo(restoring: previous, name: name)
        lastUndo = nil
        render(next)
        publish()
    }

    /// `changeFormat` on the current selection; false when nothing applies.
    @discardableResult
    public func changeFormat(_ style: KudzuCore.FormatStyle?) -> Bool {
        flushPending()
        guard !isComposing, let next = currentState.changingFormat(style) else { return false }
        perform(next, name: style == nil ? "Remove Formatting" : "Formatting")
        focus()
        return true
    }

    /// The Plain text / Formatted selector.
    public func setFormatted(_ formatted: Bool) {
        flushPending()
        guard !isComposing, formatted != state.isFormatted else { return }
        perform(currentState.settingFormatted(formatted), name: formatted ? "Formatted" : "Plain Text")
        focus()
    }

    public func toggleList(ordered: Bool) {
        flushPending()
        guard !isComposing else { return }
        perform(currentState.togglingList(ordered: ordered), name: ordered ? "Numbered List" : "Bulleted List")
        focus()
    }

    public func insertImage(dataUrl: String, width: Int, label: String) throws(EditorState.ImageError) {
        flushPending()
        guard !isComposing else { return }
        let next = try currentState.insertingImage(dataUrl: dataUrl, width: width, label: label)
        perform(next, name: "Insert Image")
        focus()
    }

    /// Replaces a source range and puts the caret after the new text.
    @discardableResult
    public func replace(_ range: Range<Int>, with text: String, focus: Bool = true) -> Range<Int> {
        if isComposing { textView.unmarkText() }
        flushPending()
        let length = state.source.utf16.count
        let clamped = min(range.lowerBound, length)..<min(max(range.lowerBound, range.upperBound), length)
        perform(currentState.replacing(clamped, with: text), name: "Typing")
        if focus { self.focus() }
        return clamped.lowerBound..<(clamped.lowerBound + text.utf16.count)
    }

    /// Inserts at the selection (replacing it).
    @discardableResult
    public func insert(_ text: String) -> Range<Int> {
        if isComposing { textView.unmarkText() }
        flushPending()
        return replace(currentState.selection, with: text, focus: true)
    }

    /// Selects a source range; ends inside a chip or image snap outward.
    public func select(_ range: Range<Int>) {
        flushPending()
        let storageRange = map.storageRange(range)
        textView.setSelectedRange(NSRange(location: storageRange.lowerBound, length: storageRange.count))
    }

    /// Ends an IME composition and publishes it. Autosave, navigation and delete call this first
    /// so a marked range still in the text view is part of the draft instead of being dropped.
    public func commitComposition() {
        guard isComposing else { return }
        textView.unmarkText()
        flushPending()
    }

    /// External value (switching snippets, imports): replaces the content without undo — the old
    /// history would restore another document — and without publishing it back. A composition in
    /// progress wins until it commits; applying the binding now would wipe the marked text.
    public func load(source: String, formatting: Formatting?) {
        guard !isComposing else { return }
        let spans = EditorState(source: source, formatting: formatting).spans
        guard !source.utf16.elementsEqual(state.source.utf16) || !FormattingHelpers.sameSpans(spans, state.spans) else { return }
        pending = []
        let selection = state.selection
        undoManager.removeAllActions()
        lastUndo = nil
        render(EditorState(source: source, spans: spans, selection: selection))
        onStateChange?()
    }

    /// Re-labels chips after snippet names change.
    public func refreshChips() {
        // A binding update (e.g. a referenced snippet rename) must not rebuild the storage while
        // AppKit owns an IME marked range. Reconciliation will use the current resolver on commit.
        guard !isComposing else { return }
        flushPending()
        let selection = currentState.selection
        var next = state
        next.selection = selection
        render(next)
    }

    public func focus() {
        textView.window?.makeFirstResponder(textView)
    }

    func chipClicked(_ tag: TagToken, rect: NSRect) {
        onChipClick?(tag, screenRect(rect))
    }
}
