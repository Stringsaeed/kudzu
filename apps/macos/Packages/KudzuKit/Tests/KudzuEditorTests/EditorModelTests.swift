import KudzuCore
import Testing
@testable import KudzuEditor

// Pure model: atoms, offset mapping, and the editor-state transitions from
// `scripts/tests/formatting.test.mjs` (CodeMirror state → `EditorState`).

@Suite struct EditorMapTests {
    @Test func atomsCoverEveryTagInOrder() {
        let atoms = EditorAtoms.atoms(source: "a{{date}}b{{bogus}}{{cursor}}", spans: nil)
        #expect(atoms.map(\.range) == [1..<9, 10..<19, 19..<29])
        #expect(atoms.map { $0.tag?.verbatim } == [false, true, false])
    }

    @Test func imagesWinOverTagsTheyContain() {
        let source = "[{{date}}] {{time}}"
        let spans = [FormatSpan(start: 0, end: 10, style: .image(dataUrl: PNG, width: 120))]
        let atoms = EditorAtoms.atoms(source: source, spans: spans)
        #expect(atoms.count == 2)
        #expect(atoms[0].kind == .image(dataUrl: PNG, width: 120))
        #expect(atoms[1].tag?.source == "{{time}}")
    }

    @Test func offsetsMapBothWays() {
        let map = EditorMap(source: "ab{{date}}cd{{time}}", spans: nil)
        #expect(map.storageLength == 6)
        #expect(map.storageOffset(2) == 2)
        #expect(map.storageOffset(10) == 3)
        #expect(map.storageOffset(12) == 5)
        #expect(map.storageOffset(20) == 6)
        #expect(map.sourceOffset(3) == 10)
        #expect(map.sourceOffset(5) == 12)
        #expect(map.sourceOffset(6) == 20)
        for storage in 0...6 { #expect(map.storageOffset(map.sourceOffset(storage)) == storage) }
    }

    @Test func positionsInsideAChipSnapToItsEdges() {
        let map = EditorMap(source: "ab{{date}}cd", spans: nil)
        #expect(map.storageOffset(4, bias: .before) == 2)
        #expect(map.storageOffset(4, bias: .after) == 3)
        #expect(map.storageOffset(4, bias: .nearest) == 2)
        #expect(map.storageOffset(8, bias: .nearest) == 3)
        // Selections never split a chip.
        #expect(map.snapped(1..<4) == 1..<10)
        #expect(map.snapped(5..<11) == 2..<11)
        #expect(map.snapped(4..<4) == 2..<2)
        #expect(map.snapped(9..<9) == 10..<10)
    }

    @Test func atomLookupByStorageIndex() {
        let map = EditorMap(source: "x{{date}}{{time}}", spans: nil)
        #expect(map.atom(atStorage: 0) == nil)
        #expect(map.atom(atStorage: 1)?.tag?.name == "date")
        #expect(map.atom(atStorage: 2)?.tag?.name == "time")
        #expect(map.atom(atStorage: 2).flatMap(map.storageIndex(of:)) == 2)
    }

    @Test func utf16OffsetsAroundEmojiAndCombiningMarks() {
        let source = "😀e\u{301}{{date}}👨‍👩‍👧"
        let map = EditorMap(source: source, spans: nil)
        // 😀 = 2 units, e + U+0301 = 2 units, so the chip starts at 4.
        #expect(map.atoms.first?.range == 4..<12)
        #expect(map.storageOffset(12) == 5)
        #expect(map.storageLength == source.utf16.count - 7)
    }
}

@Suite struct EditorStateTests {
    let bold = FormatSpan(start: 0, end: 5, style: .bold)

    @Test func formatEditUndoAndRedoPreserveFormattingWithTheText() throws {
        // Port of "format, edit, undo and redo preserve formatting with the text" (state part).
        let start = EditorState(source: "Hello world", spans: [], selection: 0..<5)
        let formatted = try #require(start.changingFormat(.bold))
        #expect(formatted.spans == [bold])
        let edited = formatted.applying(FormattingEdit(range: 0..<5, insert: "Hi"))
        #expect(edited.source == "Hi world")
        #expect(Formatting.formattedHtml(edited.source, spans: edited.spans ?? []).contains("<strong>Hi</strong> world"))
    }

    @Test func changeFormatTogglesAndSplits() throws {
        let state = EditorState(source: "Hello world", spans: [FormatSpan(start: 0, end: 11, style: .bold)], selection: 3..<5)
        let off = try #require(state.changingFormat(.bold))
        #expect(off.spans == [FormatSpan(start: 0, end: 3, style: .bold), FormatSpan(start: 5, end: 11, style: .bold)])
        #expect(EditorState(source: "Hi", spans: nil, selection: 0..<2).changingFormat(.bold) == nil)
        #expect(EditorState(source: "Hi", spans: [], selection: 1..<1).changingFormat(.bold) == nil)
    }

    @Test func linksReplaceLinksOfTheSameKind() throws {
        let state = EditorState(source: "Kudzu", spans: [FormatSpan(start: 0, end: 5, style: .link(url: "https://a.example"))], selection: 0..<5)
        #expect(state.linkAtSelection == "https://a.example")
        #expect(state.isActive("link"))
        let next = try #require(state.changingFormat(.link(url: "https://b.example")))
        #expect(next.spans == [FormatSpan(start: 0, end: 5, style: .link(url: "https://b.example"))])
    }

    @Test func selectionExpandsAcrossChipsAndImages() {
        let state = EditorState(source: "a {{date}} b", spans: [], selection: 3..<5)
        #expect(state.selectedRange == 2..<10)
    }

    @Test func removingFormattingAndSwitchingToPlainText() throws {
        // Port of "removing formatting and switching to plain text are undoable" (state part).
        let italic = try #require(EditorState(source: "Hi", spans: [], selection: 0..<2).changingFormat(.italic))
        #expect(italic.spans == [FormatSpan(start: 0, end: 2, style: .italic)])
        let plain = italic.settingFormatted(false)
        #expect(plain.spans == nil)
        #expect(plain.settingFormatted(true).spans == [])
    }

    @Test func typingBesideAnImageLeavesItIntact() {
        // Port of "typing beside an image leaves it intact; deleting it can be undone" (state part).
        let image = FormatSpan(start: 0, end: 6, style: .image(dataUrl: PNG, width: 120))
        let state = EditorState(source: "[Logo]", spans: [image])
        let typed = state.applying(FormattingEdit(range: 6..<6, insert: "!"))
        #expect(typed.spans == [image])
        let before = state.applying(FormattingEdit(range: 0..<0, insert: "!"))
        #expect(before.spans == [FormatSpan(start: 1, end: 7, style: .image(dataUrl: PNG, width: 120))])
        let deleted = typed.applying(FormattingEdit(range: 0..<6, insert: ""))
        #expect(deleted.source == "!")
        #expect(deleted.spans == [])
    }

    @Test func textMarksGrowAtTheirEdges() {
        let state = EditorState(source: "Hello", spans: [bold])
        #expect(state.applying(FormattingEdit(range: 5..<5, insert: "!")).spans == [FormatSpan(start: 0, end: 6, style: .bold)])
        #expect(state.applying(FormattingEdit(range: 0..<0, insert: ">")).spans == [FormatSpan(start: 0, end: 6, style: .bold)])
        #expect(state.applying(FormattingEdit(range: 0..<5, insert: "")).spans == [])
    }

    @Test func listsPreservePlainTextContentAndToggleOff() {
        // Port of "lists preserve plain-text content and can be toggled off".
        let state = EditorState(source: "One\nTwo", spans: [], selection: 0..<7)
        let numbered = state.togglingList(ordered: true)
        #expect(numbered.source == "1. One\n2. Two")
        #expect(numbered.togglingList(ordered: true).source == "One\nTwo")
        let bullets = state.togglingList(ordered: false)
        #expect(bullets.source == "• One\n• Two")
    }

    @Test func insertingAnImageUsesACleanAlternative() throws {
        let state = EditorState(source: "Hi there", spans: [], selection: 0..<2)
        let next = try state.insertingImage(dataUrl: PNG, width: 120, label: " Lo{g}o\n")
        #expect(next.source == "[Lo g o] there")
        #expect(next.spans == [FormatSpan(start: 0, end: 8, style: .image(dataUrl: PNG, width: 120))])
        #expect(next.selection == 8..<8)
        let unnamed = try EditorState(source: "", spans: []).insertingImage(dataUrl: PNG, width: 900, label: "  ")
        #expect(unnamed.source == "[Image]")
        #expect(unnamed.spans?.first?.style == .image(dataUrl: PNG, width: 512))
    }

    @Test func aSnippetHoldsAtMost512KiBOfImageData() throws {
        let big = "data:image/png;base64,iVBORw0KGgo" + String(repeating: "A", count: 196_606 - 33)
        #expect(Formatting.safeImage(big))
        var state = EditorState(source: "", spans: [])
        state = try state.insertingImage(dataUrl: big, width: 10, label: "a")
        state = try state.insertingImage(dataUrl: big, width: 10, label: "b")
        #expect(throws: EditorState.ImageError.snippetTooLarge) { try state.insertingImage(dataUrl: big, width: 10, label: "c") }
    }

    @Test func formattingIsLoadedOnlyForItsOwnSource() {
        let formatting = Formatting(source: "Hello", spans: [bold])
        #expect(EditorState(source: "Hello", formatting: formatting).spans == [bold])
        #expect(EditorState(source: "Hello!", formatting: formatting).spans == nil)
        #expect(EditorState(source: "Hello", formatting: nil).spans == nil)
    }
}
