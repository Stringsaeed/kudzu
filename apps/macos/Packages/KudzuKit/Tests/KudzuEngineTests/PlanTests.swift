import Foundation
@testable import KudzuCore
@testable import KudzuEngine
import Testing

// Port of the tests in `kudzu-engine/src/plan.rs`.
@Suite struct PlanTests {
    private func trigger(_ typed: String, _ separator: String?, _ fireMode: FireMode) -> TriggerMatch {
        TriggerMatch(
            snippetID: UUID(uuid: (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)),
            trigger: typed.lowercased(),
            typedTrigger: typed,
            deleteCount: Plan.graphemeCount(typed),
            separator: separator,
            insertion: .automatic,
            caseStyle: .asIs,
            fireMode: fireMode
        )
    }

    @Test func formattingFollowsUnicodeCaseExpansion() {
        let spans = Plan.casedFormatting(.upper, "ß hi", [FormatSpan(start: 2, end: 4, style: .bold)])
        #expect(spans[0].start == 3)
        #expect(spans[0].end == 5)
    }

    @Test func imageCaretAndUndoUseTheConfirmedClipboardFlavor() {
        let url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAJUlEQVR4nGMICLj0n5aYYdSCUQtGLRi1YNSCUQtGLRi1YGhYAACpwH6M7RqhEQAAAABJRU5ErkJggg=="
        let spans = [FormatSpan(start: 3, end: 9, style: .image(dataUrl: url, width: 120))]
        let visual = Plan.imageDelivery("Hi [Logo]!", spans, cursor: 9, observed: 5)
        #expect(visual?.text == "Hi \u{fffc}!")
        #expect(visual?.cursor == 4)
        let plain = Plan.imageDelivery("Hi [Logo]!", spans, cursor: 9, observed: 10)
        #expect(plain?.text == "Hi [Logo]!")
        #expect(plain?.cursor == 9)
        #expect(Plan.imageDelivery("Hi [Logo]!", spans, cursor: 9, observed: nil) == nil)
        #expect(Plan.imageDelivery("Hi [Logo]!", spans, cursor: 9, observed: 99) == nil)
    }

    @Test func activeImmediateSkipsTheSwallowedKey() {
        let m = trigger(";sig", nil, .immediate)
        let plan = InsertionPlan(m, text: "Best", cursorOffset: nil, mode: .active, swallowed: "g")
        #expect(plan.deleteCount == 3)
        #expect(plan.reemitSeparator == nil)
        #expect(plan.caretLeftCount == 0)
    }

    @Test func activeImmediateWithCombiningMarkMeasuresTheVisiblePart() {
        // "e" + combining acute is one grapheme; swallowing the mark leaves "e" visible.
        let m = trigger(";e\u{301}", nil, .immediate)
        #expect(m.deleteCount == 2)
        let plan = InsertionPlan(m, text: "x", cursorOffset: nil, mode: .active, swallowed: "\u{301}")
        #expect(plan.deleteCount == 2)
    }

    @Test func activeImmediateWithoutSwallowedTextFallsBack() {
        let m = trigger("abc", nil, .immediate)
        let plan = InsertionPlan(m, text: "x", cursorOffset: nil, mode: .active, swallowed: nil)
        #expect(plan.deleteCount == 2)
    }

    @Test func activeWholeWordKeepsSeparatorOutOfTheDocument() {
        let m = trigger("brb", " ", .wholeWord)
        let plan = InsertionPlan(m, text: "be right back", cursorOffset: nil, mode: .active, swallowed: " ")
        #expect(plan.deleteCount == 3)
        #expect(plan.reemitSeparator == " ")
    }

    @Test func listenOnlyDeletesTheSeparatorToo() {
        let m = trigger("brb", ".", .wholeWord)
        let plan = InsertionPlan(m, text: "be right back", cursorOffset: nil, mode: .listenOnly, swallowed: nil)
        #expect(plan.deleteCount == 4)
        let immediate = trigger(";x", nil, .immediate)
        let second = InsertionPlan(immediate, text: "y", cursorOffset: nil, mode: .listenOnly, swallowed: "x")
        #expect(second.deleteCount == 2)
    }

    @Test func caretCountsGraphemesAfterCursorPlusSeparator() {
        let m = trigger("sig", " ", .wholeWord)
        // "Hi 👋🏽 there", cursor after "Hi " (3 UTF-16 units).
        let plan = InsertionPlan(m, text: "Hi 👋🏽 there", cursorOffset: 3, mode: .active, swallowed: nil)
        #expect(plan.caretLeftCount == 7 + 1)
    }

    @Test func caretInsideAGraphemeSnapsToItsStart() {
        // 👋🏽 is 4 UTF-16 units; an offset of 2 lands inside it.
        #expect(Plan.graphemesAfterUTF16("👋🏽a", 2) == 2)
        #expect(Plan.graphemesAfterUTF16("ab", 0) == 2)
        #expect(Plan.graphemesAfterUTF16("ab", 5) == 0)
        #expect(Plan.graphemesAfterUTF16("a\r\nb", 2) == 2)
    }

    @Test func utf16PrefixNeverSplitsSurrogates() {
        #expect(Plan.utf16Prefix("a😀b", 1) == "a")
        #expect(Plan.utf16Prefix("a😀b", 2) == "a")
        #expect(Plan.utf16Prefix("a😀b", 3) == "a😀")
        #expect(Plan.utf16Prefix("a😀b", 99) == "a😀b")
    }

    @Test func asIsCursorOffsetIsUnchanged() {
        #expect(Plan.casedCursorOffset(.asIs, "abc", 2) == 2)
    }

    @Test func charsSavedNeverNegative() {
        #expect(Plan.charsSaved("Best regards", ";sig") == 8)
        #expect(Plan.charsSaved("a", ";long") == 0)
        #expect(Plan.charsSaved("👋🏽👋🏽", "w") == 1)
    }

    @Test func automaticTypesShortSingleLineText() {
        #expect(Plan.shouldType(.automatic, "hello"))
        #expect(Plan.shouldType(.automatic, "one\ntwo"))
        #expect(!Plan.shouldType(.automatic, "one\ntwo\nthree"))
        #expect(!Plan.shouldType(.automatic, "one\r\ntwo\r\nthree"))
        #expect(!Plan.shouldType(.automatic, String(repeating: "x", count: 100)))
        #expect(Plan.shouldType(.automatic, String(repeating: "x", count: 99)))
        #expect(Plan.shouldType(.type, String(repeating: "x", count: 500)))
        #expect(!Plan.shouldType(.paste, "x"))
    }
}
