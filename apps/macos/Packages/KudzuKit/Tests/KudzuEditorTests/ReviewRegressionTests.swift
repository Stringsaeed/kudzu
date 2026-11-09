import AppKit
import KudzuCore
import Testing
@testable import KudzuEditor

@MainActor
@Suite(.serialized) struct ReviewRegressionTests {
    @Test func nativePasteDoesNotOpenSlashMenu() {
        let (editor, recorder) = makeEditor("")
        let board = NSPasteboard(name: .init("kz-review-\(UUID().uuidString)"))
        defer { board.releaseGlobally() }
        board.setString("/", forType: .string)
        #expect(editor.textView.readSelection(from: board, type: .string))
        #expect(recorder.value == "/")
        #expect(recorder.slashes.isEmpty)
    }

    @Test func typedSlashReplacingSelectionOpensMenu() {
        let (editor, recorder) = makeEditor("word", selection: 0..<4)
        editor.type("/")
        #expect(recorder.value == "/")
        #expect(recorder.slashes.map(\.position) == [0])
    }

    @Test func markedTextNextToChipCommitsOnceAndCanBeUndone() {
        let (editor, recorder) = makeEditor("{{date}}", selection: 8..<8)
        editor.textView.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        #expect(editor.textView.hasMarkedText())
        #expect(recorder.changes == 0)
        editor.textView.setMarkedText("日本", selectedRange: NSRange(location: 2, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        editor.textView.unmarkText()
        #expect(recorder.value == "{{date}}日本")
        #expect(editor.state.source == "{{date}}日本")
        #expect(recorder.changes == 1)
        editor.undoManager.undo()
        #expect(recorder.value == "{{date}}")
    }

    @Test func markedTextOverChipPreservesAtomicReplacement() {
        let (editor, recorder) = makeEditor("a{{date}}{{time}}z", selection: 1..<9)
        editor.textView.setMarkedText("あ", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        editor.textView.insertText("愛", replacementRange: NSRange(location: NSNotFound, length: 0))
        #expect(recorder.value == "a愛{{time}}z")
        #expect(editor.map.atoms.count == 1)
        editor.undoManager.undo()
        #expect(recorder.value == "a{{date}}{{time}}z")
    }

    @Test func attachmentsHaveMeaningfulAccessibilityLabels() throws {
        let (editor, _) = makeEditor("{{date}}[Logo]", spans: [FormatSpan(start: 8, end: 14, style: .image(dataUrl: PNG, width: 64))])
        let cells = editor.scan().atoms.compactMap { $0.attachment.attachmentCell as? NSTextAttachmentCell }
        #expect(cells.count == 2)
        #expect(cells[0].accessibilityLabel() == "Date · ISO date — click to edit")
        #expect(cells[1].accessibilityLabel() == "[Logo]")
    }

    @Test func imageLabelReplacesEachBraceAndLineBreakLikeJavaScript() throws {
        let state = EditorState(source: "", spans: [])
        let next = try state.insertingImage(dataUrl: PNG, width: 64, label: "A\r\nB{\u{301}}")
        #expect(utf16Equal(next.source, "[A  B \u{301} ]"))
    }
    @Test func literalObjectReplacementCharacterBesideChipStaysLiteral() {
        let (editor, recorder) = makeEditor("{{date}}", selection: 8..<8)
        editor.type("\u{FFFC}")
        #expect(utf16Equal(recorder.value, "{{date}}\u{FFFC}"))
        #expect(editor.map.atoms.count == 1)
    }

    @Test func nativeReplaceAllPreservesStylesBetweenDisjointEdits() {
        let (editor, recorder) = makeEditor("a{{date}}Bz", spans: [FormatSpan(start: 9, end: 10, style: .bold)])
        let ranges = [NSRange(location: 0, length: 1), NSRange(location: 3, length: 1)]
        #expect(editor.textView.shouldChangeText(inRanges: ranges.map(NSValue.init(range:)), replacementStrings: ["aa", "zz"]))
        editor.storage.beginEditing()
        editor.textView.replaceCharacters(in: ranges[1], with: "zz")
        editor.textView.replaceCharacters(in: ranges[0], with: "aa")
        editor.storage.endEditing()
        editor.textView.didChangeText()
        #expect(recorder.value == "aa{{date}}Bzz")
        #expect(recorder.spans == [FormatSpan(start: 10, end: 11, style: .bold)])
        editor.undoManager.undo()
        #expect(recorder.value == "a{{date}}Bz")
    }

    @Test func relabelingChipsDoesNotDiscardMarkedText() {
        let (editor, recorder) = makeEditor("{{date}}", selection: 8..<8)
        editor.textView.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        editor.refreshChips()
        #expect(editor.textView.hasMarkedText())
        editor.textView.insertText("日本", replacementRange: NSRange(location: NSNotFound, length: 0))
        #expect(recorder.value == "{{date}}日本")
    }

    @Test func nativeReplacementAfterEmojiAndAdjacentChipsUsesSourceOffsets() {
        let (editor, recorder) = makeEditor("😀{{date}}{{time}}e\u{301}", spans: [FormatSpan(start: 18, end: 20, style: .link(url: "https://kudzu.site"))])
        let range = NSRange(location: 4, length: 2)
        #expect(editor.textView.shouldChangeText(in: range, replacementString: "text"))
        editor.textView.replaceCharacters(in: range, with: "text")
        editor.textView.didChangeText()
        #expect(recorder.value == "😀{{date}}{{time}}text")
        #expect(recorder.spans == [FormatSpan(start: 18, end: 22, style: .link(url: "https://kudzu.site"))])
    }

    @Test(arguments: [("\u{FEFF}", true), ("\u{0085}", false), ("\u{2028}", true), ("\u{00A0}", true)])
    func slashUsesJavaScriptWhitespace(prefix: String, opens: Bool) {
        let (editor, recorder) = makeEditor(prefix)
        editor.type("/")
        #expect(!recorder.slashes.isEmpty == opens)
    }

    @Test func actualCommandKeyEventsApplyAllThreeStyles() throws {
        let (editor, recorder) = makeEditor("{{date}}", spans: [], selection: 0..<8)
        let window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: 480, height: 240), styleMask: [.titled], backing: .buffered, defer: true)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        window.contentView = editor.scrollView
        #expect(window.makeFirstResponder(editor.textView))
        for key in ["b", "i", "u"] {
            let event = try #require(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0, windowNumber: window.windowNumber, context: nil, characters: key, charactersIgnoringModifiers: key, isARepeat: false, keyCode: 0))
            #expect(editor.textView.performKeyEquivalent(with: event))
        }
        #expect(recorder.spans == [FormatSpan(start: 0, end: 8, style: .bold), FormatSpan(start: 0, end: 8, style: .italic), FormatSpan(start: 0, end: 8, style: .underline)])
    }

    @Test func nativeAccessibilityExposesAttachmentDescriptions() {
        let (editor, _) = makeEditor("{{date}}[Logo]", spans: [FormatSpan(start: 8, end: 14, style: .image(dataUrl: PNG, width: 64))])
        let attributed = editor.textView.accessibilityAttributedString(for: NSRange(location: 0, length: 2))
        let children = editor.textView.accessibilityChildren()?.compactMap { $0 as? NSObjectProtocol } ?? []
        #expect(children.map { $0.perform(NSSelectorFromString("accessibilityAttributeValue:"), with: "AXDescription")?.takeUnretainedValue() as? String } == ["Date · ISO date — click to edit", "[Logo]"])
        #expect(attributed?.attribute(NSAttributedString.Key("AXAttachment"), at: 0, effectiveRange: nil) != nil)
        #expect(attributed?.attribute(NSAttributedString.Key("AXAttachment"), at: 1, effectiveRange: nil) != nil)
    }

    @Test func commitCompositionPublishesMarkedTextForTheDraft() {
        let (editor, recorder) = makeEditor("{{date}}", selection: 8..<8)
        editor.textView.setMarkedText("日本", selectedRange: NSRange(location: 2, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        #expect(recorder.changes == 0)
        #expect(editor.isComposing)
        editor.commitComposition()
        #expect(recorder.value == "{{date}}日本")
        #expect(!editor.isComposing)
        #expect(recorder.changes == 1)
        // A second call is a no-op: the composition is already in the draft.
        editor.commitComposition()
        #expect(recorder.changes == 1)
    }

    @Test func loadWhileComposingLeavesTheMarkedTextAlone() {
        let (editor, recorder) = makeEditor("{{date}}", selection: 8..<8)
        editor.textView.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        editor.load(source: "replaced", formatting: nil)
        #expect(editor.textView.hasMarkedText())
        #expect(recorder.value == "{{date}}")
        editor.commitComposition()
        #expect(recorder.value == "{{date}}に")
    }

    @Test func handleInsertCommitsIMEBeforeCreatingItsOwnUndoStep() {
        let (editor, recorder) = makeEditor("{{date}}", selection: 8..<8)
        editor.textView.setMarkedText("日本", selectedRange: NSRange(location: 2, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        editor.insert("!")
        #expect(recorder.value == "{{date}}日本!")
        #expect(!editor.textView.hasMarkedText())
        editor.undoManager.undo()
        #expect(recorder.value == "{{date}}日本")
        editor.undoManager.undo()
        #expect(recorder.value == "{{date}}")
    }

    @Test func chipEditsPreserveBoldAndLinkSpanEdges() {
        let (editor, recorder) = makeEditor("a{{date}}bc", spans: [FormatSpan(start: 1, end: 10, style: .bold)], selection: 1..<9)
        editor.textView.deleteBackward(nil)
        #expect(recorder.value == "abc")
        #expect(recorder.spans == [FormatSpan(start: 1, end: 2, style: .bold)])
        editor.select(1..<1)
        editor.type("{{time}}")
        #expect(recorder.spans == [FormatSpan(start: 1, end: 10, style: .bold)])
        // Delete across the leading edge of the bold span, retaining only its final character.
        editor.select(0..<9)
        editor.textView.deleteBackward(nil)
        #expect(recorder.value == "bc")
        #expect(recorder.spans == [FormatSpan(start: 0, end: 1, style: .bold)])
        editor.select(0..<1)
        #expect(editor.changeFormat(.link(url: "https://kudzu.site")))
        editor.type("new")
        #expect(recorder.spans == [FormatSpan(start: 0, end: 3, style: .bold), FormatSpan(start: 0, end: 3, style: .link(url: "https://kudzu.site"))])
    }

}
