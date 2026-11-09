import AppKit
import KudzuCore
import Testing
@testable import KudzuEditor

// The real NSTextView, headless: storage ↔ source round trips, chip atomicity, typing, undo.

let corpus: [String] = [
    "",
    "Plain text only",
    "Hello {{date}} world",
    #"{{date format="EEEE, MMMM d" offset="+1d"}} {{time format="HH:mm"}}"#,
    "{{clipboard}} {{clipboard trim=true}} {{cursor}}",
    #"{{snippet id="00000000-0000-0000-0000-000000000001"}}"#,
    #"{{field name="who" default="Sam" multiline=true}} {{choice name="size" options="S|M|L\|XL" default="M"}}"#,
    "{{unknown}} {{date format=}} {{Date}} {{field}}",
    #"{{field name="a }} b"}}"#,
    #"\{{literal}} \\{{date}} \\\{{x}} {{{date}}"#,
    "{{ unclosed and }} stray }} and {{",
    "{{date}}{{time}}{{cursor}}",
    "😀 {{date}} 👨‍👩‍👧 e\u{301}\u{302} {{time}}e\u{301}",
    "سلمى {{field name=\"اسم\"}} עברית",
    "Tab\there\r\nCRLF\nLF\u{2028}",
    "Literal \u{FFFC} object replacement {{cursor}}",
    "\u{1F1FA}\u{1F1F8}{{clipboard}}\u{0301}",
]

@MainActor
@Suite struct RoundTripTests {
    @Test(arguments: corpus)
    func loadingKeepsTheSourceByteForByte(source: String) {
        let (editor, _) = makeEditor(source)
        #expect(utf16Equal(editor.serialized, source))
        let tags = Template.tokenize(source).filter { if case .tag = $0 { true } else { false } }.count
        #expect(editor.map.atoms.count == tags)
        #expect(editor.storage.length == editor.map.storageLength)
    }

    @Test(arguments: corpus)
    func typingKeepsTheSourceByteForByte(source: String) {
        let (editor, recorder) = makeEditor("")
        editor.type(source)
        #expect(utf16Equal(editor.serialized, source))
        #expect(utf16Equal(recorder.value, source) || source.isEmpty)
        #expect(utf16Equal(editor.state.source, source))
    }

    @Test func formattedRoundTrip() {
        let source = "[Logo] Hello {{field name=who}}!"
        let spans = [
            FormatSpan(start: 0, end: 6, style: .image(dataUrl: PNG, width: 120)),
            FormatSpan(start: 7, end: 12, style: .bold),
            FormatSpan(start: 13, end: 31, style: .italic),
        ]
        let (editor, _) = makeEditor(source, spans: spans)
        #expect(editor.storage.length == source.utf16.count - 5 - 17)
        #expect(utf16Equal(editor.serialized, source))
        #expect(editor.state.spans == spans)
    }

    @Test func copyWritesTheSourceNotAttachmentCharacters() {
        let (editor, _) = makeEditor("Hi {{date}} there")
        editor.textView.selectAll(nil)
        let board = NSPasteboard(name: NSPasteboard.Name("kz-editor-test-\(UUID().uuidString)"))
        defer { board.releaseGlobally() }
        #expect(editor.textView.writeSelection(to: board, types: [.string]))
        #expect(board.string(forType: .string) == "Hi {{date}} there")
    }

    @Test func pastedTagsBecomeChips() {
        let (editor, recorder) = makeEditor("A")
        editor.setSourceSelection(1..<1)
        editor.textView.insertText(" {{cursor}}", replacementRange: NSRange(location: NSNotFound, length: 0))
        #expect(recorder.value == "A {{cursor}}")
        #expect(editor.map.atoms.count == 1)
        #expect(editor.storageString == "A \u{FFFC}")
    }
}

@MainActor
@Suite struct ChipAtomicityTests {
    @Test func arrowKeysStepOverAChipAsOneUnit() {
        let (editor, _) = makeEditor("a{{date}}b", selection: 1..<1)
        editor.textView.moveRight(nil)
        #expect(editor.sourceSelectionNow == 9..<9)
        editor.textView.moveLeft(nil)
        #expect(editor.sourceSelectionNow == 1..<1)
        editor.textView.moveRightAndModifySelection(nil)
        #expect(editor.sourceSelectionNow == 1..<9)
    }

    @Test func oneBackspaceRemovesTheWholeTag() {
        let (editor, recorder) = makeEditor("x{{date format=\"EEEE\"}}y", selection: 23..<23)
        editor.textView.deleteBackward(nil)
        #expect(recorder.value == "xy")
        #expect(editor.sourceSelectionNow == 1..<1)
        editor.undoManager.undo()
        #expect(recorder.value == "x{{date format=\"EEEE\"}}y")
        #expect(editor.map.atoms.count == 1)
        #expect(editor.sourceSelectionNow == 23..<23)
    }

    @Test func forwardDeleteRemovesTheWholeTag() {
        let (editor, recorder) = makeEditor("{{cursor}}!", selection: 0..<0)
        editor.textView.deleteForward(nil)
        #expect(recorder.value == "!")
    }

    @Test func programmaticSelectionsSnapAroundChips() {
        let (editor, recorder) = makeEditor("ab{{date}}cd")
        editor.select(4..<4)
        #expect(editor.sourceSelectionNow == 2..<2)
        editor.select(8..<8)
        #expect(editor.sourceSelectionNow == 10..<10)
        editor.select(3..<11)
        #expect(editor.sourceSelectionNow == 2..<11)
        #expect(recorder.selections.last == 2..<11)
    }

    @Test func typingTheClosingBracesTurnsTextIntoAChip() {
        let (editor, recorder) = makeEditor("Today: ")
        editor.type("{{date")
        #expect(editor.map.atoms.isEmpty)
        editor.type("}}")
        #expect(editor.map.atoms.count == 1)
        #expect(editor.storageString == "Today: \u{FFFC}")
        #expect(editor.sourceSelectionNow == 15..<15)
        editor.type("!")
        #expect(recorder.value == "Today: {{date}}!")
    }

    @Test func escapingAChipTurnsItBackIntoText() {
        let (editor, recorder) = makeEditor("{{date}}", selection: 0..<0)
        editor.type("\\")
        #expect(recorder.value == "\\{{date}}")
        #expect(editor.map.atoms.isEmpty)
        #expect(editor.storageString == "\\{{date}}")
    }

    @Test func chipsUseTheirLabelsAndTones() throws {
        let id = "00000000-0000-0000-0000-000000000001"
        let (editor, _) = makeEditor(#"{{date}}{{snippet id="\#(id)"}}{{nope}}"#)
        editor.snippetName = { $0 == id ? "Signature" : nil }
        editor.refreshChips()
        let cells = editor.scan().atoms.compactMap { $0.attachment.attachmentCell as? ChipCell }
        #expect(cells.map(\.label) == ["Date · ISO date", "↳ Signature", "{{nope}}"])
        #expect(cells.map(\.tone) == [.time, .snippet, .error])
        #expect(cells[2].labelFont.isFixedPitch)
        let size = cells[0].cellSize()
        #expect(size.height == 20)
        let tip = editor.storage.attribute(.toolTip, at: 2, effectiveRange: nil) as? String
        #expect(tip == "Not recognised: {{nope}}")
    }
}

@MainActor
@Suite struct UndoTests {
    @Test func formatEditUndoAndRedo() {
        // Port of "format, edit, undo and redo preserve formatting with the text".
        let (editor, recorder) = makeEditor("Hello world", spans: [], selection: 0..<5)
        #expect(editor.changeFormat(.bold))
        editor.replace(0..<5, with: "Hi")
        #expect(Formatting.formattedHtml(recorder.value, spans: recorder.spans ?? []).contains("<strong>Hi</strong> world"))
        editor.undoManager.undo()
        #expect(recorder.value == "Hello world")
        #expect(recorder.spans == [FormatSpan(start: 0, end: 5, style: .bold)])
        editor.undoManager.undo()
        #expect(recorder.spans == [])
        editor.undoManager.redo()
        #expect(recorder.spans == [FormatSpan(start: 0, end: 5, style: .bold)])
    }

    @Test func removingFormattingAndSwitchingToPlainTextAreUndoable() {
        // Port of "removing formatting and switching to plain text are undoable".
        let (editor, recorder) = makeEditor("Hi", spans: [], selection: 0..<2)
        #expect(editor.changeFormat(.italic))
        editor.setFormatted(false)
        #expect(recorder.spans == nil)
        editor.undoManager.undo()
        #expect(recorder.spans == [FormatSpan(start: 0, end: 2, style: .italic)])
    }

    @Test func typingBesideAnImageLeavesItIntactAndDeletingItCanBeUndone() {
        // Port of "typing beside an image leaves it intact; deleting it can be undone".
        let image = FormatSpan(start: 0, end: 6, style: .image(dataUrl: PNG, width: 120))
        let (editor, recorder) = makeEditor("[Logo]", spans: [image])
        #expect(editor.storageString == "\u{FFFC}")
        editor.type("!")
        #expect(recorder.spans == [image])
        editor.textView.setSelectedRange(NSRange(location: 0, length: 1))
        editor.textView.deleteBackward(nil)
        #expect(recorder.value == "!")
        #expect(recorder.spans == [])
        editor.undoManager.undo()
        #expect(recorder.value == "[Logo]!")
        #expect(recorder.spans == [image])
        #expect(editor.storageString == "\u{FFFC}!")
    }

    @Test func removeFormattingReplacesAnImageWithItsAlternative() {
        let image = FormatSpan(start: 0, end: 6, style: .image(dataUrl: PNG, width: 120))
        let (editor, recorder) = makeEditor("[Logo] hi", spans: [image])
        editor.textView.setSelectedRange(NSRange(location: 0, length: 1))
        #expect(editor.changeFormat(nil))
        #expect(recorder.spans == [])
        #expect(editor.storageString == "[Logo] hi")
        editor.undoManager.undo()
        #expect(editor.storageString == "\u{FFFC} hi")
    }

    @Test func plainTextKeepsImageAlternativesAndUndoRestoresTheImage() {
        let image = FormatSpan(start: 3, end: 9, style: .image(dataUrl: PNG, width: 64))
        let (editor, recorder) = makeEditor("Hi [Logo]", spans: [image])
        editor.setFormatted(false)
        #expect(recorder.value == "Hi [Logo]")
        #expect(recorder.spans == nil)
        #expect(editor.storageString == "Hi [Logo]")
        #expect(editor.textView.font?.isFixedPitch == true || (editor.storage.attribute(.font, at: 0, effectiveRange: nil) as? NSFont)?.isFixedPitch == true)
        editor.undoManager.undo()
        #expect(recorder.spans == [image])
        #expect(editor.storageString == "Hi \u{FFFC}")
    }

    @Test func typingCoalescesIntoOneUndoStep() {
        let (editor, recorder) = makeEditor("")
        editor.type("abc")
        #expect(recorder.value == "abc")
        editor.undoManager.undo()
        #expect(recorder.value == "")
        editor.undoManager.redo()
        #expect(recorder.value == "abc")
    }

    @Test func movingTheCaretStartsANewUndoStep() {
        let (editor, recorder) = makeEditor("")
        editor.type("ab")
        editor.textView.moveLeft(nil)
        editor.type("X")
        #expect(recorder.value == "aXb")
        editor.undoManager.undo()
        #expect(recorder.value == "ab")
        editor.undoManager.undo()
        #expect(recorder.value == "")
    }

    @Test func listsAreOneUndoStep() {
        // Port of "lists preserve plain-text content and can be toggled off".
        let (editor, recorder) = makeEditor("One\nTwo", spans: [], selection: 0..<7)
        editor.toggleList(ordered: true)
        #expect(recorder.value == "1. One\n2. Two")
        editor.select(0..<13)
        editor.toggleList(ordered: true)
        #expect(recorder.value == "One\nTwo")
        editor.undoManager.undo()
        #expect(recorder.value == "1. One\n2. Two")
    }

    @Test func externalValuesReplaceContentAndClearHistory() {
        let (editor, recorder) = makeEditor("old")
        editor.type("!")
        #expect(editor.undoManager.canUndo)
        let changes = recorder.changes
        editor.load(source: "new {{date}}", formatting: nil)
        #expect(editor.serialized == "new {{date}}")
        #expect(!editor.undoManager.canUndo)
        #expect(recorder.changes == changes)
    }
}

@MainActor
@Suite struct FormattingViewTests {
    @Test func spansRenderAsAttributes() throws {
        let spans = [
            FormatSpan(start: 0, end: 4, style: .bold),
            FormatSpan(start: 2, end: 6, style: .italic),
            FormatSpan(start: 7, end: 9, style: .underline),
            FormatSpan(start: 10, end: 14, style: .link(url: "https://kudzu.site")),
        ]
        let (editor, _) = makeEditor("abcdef gh link", spans: spans)
        let storage = editor.storage
        func traits(_ i: Int) -> NSFontTraitMask {
            NSFontManager.shared.traits(of: try! #require(storage.attribute(.font, at: i, effectiveRange: nil) as? NSFont))
        }
        #expect(traits(0).contains(.boldFontMask) && !traits(0).contains(.italicFontMask))
        #expect(traits(3).contains(.boldFontMask) && traits(3).contains(.italicFontMask))
        #expect(traits(5).contains(.italicFontMask) && !traits(5).contains(.boldFontMask))
        #expect(storage.attribute(.underlineStyle, at: 7, effectiveRange: nil) as? Int == NSUnderlineStyle.single.rawValue)
        #expect(storage.attribute(.underlineStyle, at: 6, effectiveRange: nil) == nil)
        #expect(storage.attribute(.toolTip, at: 11, effectiveRange: nil) as? String == "https://kudzu.site")
        #expect(!(try #require(storage.attribute(.font, at: 6, effectiveRange: nil) as? NSFont)).isFixedPitch)
    }

    @Test func keyboardShortcutStyleToggleWorksOnlyInFormattedMode() {
        let (plain, _) = makeEditor("Hi", selection: 0..<2)
        let applied = plain.changeFormat(.bold)
        #expect(applied == false)
        #expect(plain.state.spans == nil)
        let (formatted, recorder) = makeEditor("Hi", spans: [], selection: 0..<2)
        #expect(formatted.changeFormat(.bold))
        #expect(formatted.changeFormat(.bold))
        #expect(recorder.spans == [])
    }

    @Test func typingInsideABoldSpanIsBold() {
        let (editor, recorder) = makeEditor("Hello", spans: [FormatSpan(start: 0, end: 5, style: .bold)], selection: 5..<5)
        editor.type("!")
        #expect(recorder.spans == [FormatSpan(start: 0, end: 6, style: .bold)])
        let font = editor.storage.attribute(.font, at: 5, effectiveRange: nil) as? NSFont
        #expect(font.map { NSFontManager.shared.traits(of: $0).contains(.boldFontMask) } == true)
    }

    @Test func imagesAreAttachmentsAtTheirDisplayWidth() throws {
        let (editor, _) = makeEditor("[Logo]", spans: [FormatSpan(start: 0, end: 6, style: .image(dataUrl: PNG, width: 120))])
        let cell = try #require(editor.scan().atoms.first?.attachment.attachmentCell as? ImageCell)
        #expect(cell.cellSize() == NSSize(width: 120, height: 120))
        let rect = try #require(editor.glyphRect(atStorage: 0))
        #expect(abs(rect.width - 120) < 0.5)
    }

    @Test func insertImageIsUndoable() throws {
        let (editor, recorder) = makeEditor("Hi", spans: [], selection: 2..<2)
        try editor.insertImage(dataUrl: PNG, width: 64, label: "Logo")
        #expect(recorder.value == "Hi[Logo]")
        #expect(editor.storageString == "Hi\u{FFFC}")
        editor.undoManager.undo()
        #expect(recorder.value == "Hi")
        #expect(recorder.spans == [])
    }

    @Test func linesAre1Point7Tall() throws {
        let (editor, _) = makeEditor("one\ntwo")
        let layout = try #require(editor.layoutManager)
        layout.ensureLayout(for: try #require(editor.textView.textContainer))
        let line = layout.lineFragmentRect(forGlyphAt: 0, effectiveRange: nil)
        #expect(abs(line.height - 22.1) < 0.01)
        #expect(editor.textView.textContainerInset == NSSize(width: 14, height: 12))
        #expect((editor.storage.attribute(.font, at: 0, effectiveRange: nil) as? NSFont)?.isFixedPitch == true)
    }
}

@MainActor
@Suite struct CallbackTests {
    @Test func slashAtAWordStartOpensTheMenu() throws {
        let (editor, recorder) = makeEditor("")
        editor.type("/")
        #expect(recorder.slashes.map(\.position) == [0])
        editor.type("x a/")
        #expect(recorder.slashes.count == 1)
        editor.type(" (/")
        #expect(recorder.slashes.map(\.position) == [0, 7])
        let rect = try #require(recorder.slashes.last?.rect)
        let caret = try #require(editor.viewRect(at: 7))
        #expect(rect == caret)
    }

    @Test func handleInsertAndReplaceReturnRanges() {
        let (editor, recorder) = makeEditor("Hi /", selection: 4..<4)
        let replaced = editor.replace(3..<4, with: "{{date}}")
        #expect(replaced == 3..<11)
        #expect(recorder.value == "Hi {{date}}")
        #expect(editor.sourceSelectionNow == 11..<11)
        let inserted = editor.insert(" {{cursor}}")
        #expect(inserted == 11..<22)
        #expect(recorder.value == "Hi {{date}} {{cursor}}")
        #expect(editor.map.atoms.count == 2)
    }

    @Test func coordsAtFollowsChipEdges() throws {
        let (editor, _) = makeEditor("ab{{date}}cd")
        let before = try #require(editor.viewRect(at: 2))
        let after = try #require(editor.viewRect(at: 10))
        let chip = try #require(editor.chipRect(at: 2))
        #expect(abs(before.minX - chip.minX) < 0.5)
        #expect(abs(after.minX - chip.maxX) < 0.5)
        #expect(after.minX - before.minX > 30)
    }

    @Test func clickingAChipReportsTheTagAndItsScreenRect() throws {
        let (editor, recorder) = makeEditor("Hi {{clipboard}} there")
        let window = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 480, height: 240), styleMask: [.borderless], backing: .buffered, defer: true)
        window.contentView = editor.scrollView
        editor.scrollView.frame = window.contentView?.bounds ?? .zero
        let chip = try #require(editor.chipRect(at: 3))
        let inWindow = editor.textView.convert(NSPoint(x: chip.midX, y: chip.midY), to: nil)
        let event = try #require(NSEvent.mouseEvent(with: .leftMouseDown, location: inWindow, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1))
        editor.textView.mouseDown(with: event)
        #expect(recorder.chips.count == 1)
        #expect(recorder.chips.first?.tag.source == "{{clipboard}}")
        #expect(recorder.chips.first?.tag.from == 3)
        let expected = window.convertToScreen(editor.textView.convert(chip, to: nil))
        #expect(recorder.chips.first?.rect == expected)
        // The caret did not move onto the chip.
        #expect(editor.sourceSelectionNow == 22..<22)
    }

    @Test func clickingAnImageSelectsIt() throws {
        let (editor, _) = makeEditor("a[Logo]b", spans: [FormatSpan(start: 1, end: 7, style: .image(dataUrl: PNG, width: 40))])
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 480, height: 240), styleMask: [.borderless], backing: .buffered, defer: true)
        window.contentView = editor.scrollView
        let image = try #require(editor.glyphRect(atStorage: 1))
        let point = editor.textView.convert(NSPoint(x: image.midX, y: image.midY), to: nil)
        let event = try #require(NSEvent.mouseEvent(with: .leftMouseDown, location: point, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1))
        editor.textView.mouseDown(with: event)
        #expect(editor.sourceSelectionNow == 1..<7)
    }
}
