import Foundation
import Testing
@testable import KudzuCore

struct InsertionPlanTests {
    private func match(_ mode: FireMode) -> TriggerMatch {
        TriggerMatch(snippetID: UUID(), trigger: ";kz", typedTrigger: ";kz",
                     deleteCount: 3, separator: mode == .wholeWord ? "." : nil,
                     insertion: .automatic, caseStyle: .asIs, fireMode: mode)
    }

    @Test func swallowedCombiningMarkLeavesTheBaseCharacterToDelete() {
        // Trigger "é" typed as "e" + combining acute: the acute is withheld, so the document
        // still shows "e", one grapheme that must be deleted.
        let typed = "e\u{0301}"
        let match = TriggerMatch(snippetID: UUID(), trigger: typed, typedTrigger: typed, deleteCount: 1,
                                 separator: nil, insertion: .automatic, caseStyle: .asIs, fireMode: .immediate)
        let plan = InsertionPlan(match: match, text: "x", cursorOffset: nil, mode: .active, swallowedText: "\u{0301}")
        #expect(plan.deleteCount == 1)
    }

    @Test func swallowedPlainKeyDeletesTheRestOfTheTrigger() {
        let plan = InsertionPlan(match: match(.immediate), text: "x", cursorOffset: nil, mode: .active, swallowedText: "z")
        #expect(plan.deleteCount == 2)
    }

    @Test func deletionAndSeparator() {
        for mode in [ExpansionTapMode.active, .listenOnly] {
            let immediate = InsertionPlan(match: match(.immediate), text: "Hello", cursorOffset: nil, mode: mode)
            #expect(immediate.deleteCount == (mode == .active ? 2 : 3))
            #expect(immediate.reemitSeparator == nil)
            let word = InsertionPlan(match: match(.wholeWord), text: "Hello", cursorOffset: nil, mode: mode)
            #expect(word.deleteCount == (mode == .active ? 3 : 4))
            #expect(word.reemitSeparator == ".")
            #expect(word.text == "Hello")
            #expect(word.caretLeftCount == 0)
        }
    }

    @Test(arguments: [
        ("a👨‍👩‍👧‍👦🇦🇪", 1, 2),
        ("ae\u{301}z", 1, 2),
        ("a\r\nb", 1, 2),
        ("a\r\nb", 2, 2),
        ("ae\u{301}z", 2, 2),
        ("a👨‍👩‍👧‍👦🇦🇪", 2, 2),
        ("👋🏽x", 0, 2),
        ("abc", 3, 0),
        ("", 0, 0),
        ("abc", 99, 0),
        ("abc", -1, 3)
    ])
    func caret(text: String, offset: Int, expected: Int) {
        let plan = InsertionPlan(match: match(.immediate), text: text, cursorOffset: offset, mode: .active)
        #expect(plan.caretLeftCount == expected)
    }

    @Test func separatorIsAfterCursor() {
        let plan = InsertionPlan(match: match(.wholeWord), text: "ab", cursorOffset: 1, mode: .active)
        #expect(plan.caretLeftCount == 2)
    }

    @Test func queueLifecycle() {
        var queue = HoldReplayQueue<Int>()
        let idleAccepted = queue.enqueue(0)
        #expect(!idleAccepted)
        queue.begin()
        #expect(queue.state == .inserting)
        let firstAccepted = queue.enqueue(1)
        let secondAccepted = queue.enqueue(2)
        #expect(firstAccepted && secondAccepted)
        let first = queue.next()
        #expect(first == 1)
        let thirdAccepted = queue.enqueue(3)
        #expect(thirdAccepted)
        let remainder = queue.drain()
        #expect(remainder == [2, 3])
        #expect(queue.state == .idle)
        let empty = queue.drain()
        #expect(empty.isEmpty)
    }

    @Test func timeoutAndRepeatedBeginPreserveOrder() {
        var queue = HoldReplayQueue<String>()
        queue.begin()
        _ = queue.enqueue("down")
        queue.begin()
        _ = queue.enqueue("up")
        let timedOut = queue.timeout()
        #expect(timedOut == ["down", "up"])
        #expect(queue.state == .idle)
        queue.begin()
        let accepted = queue.enqueue("new")
        #expect(accepted)
        let first = queue.next()
        let end = queue.next()
        #expect(first == "new")
        #expect(end == nil)
        #expect(queue.state == .idle)
    }

    @Test func triggerDuringInsertionStartsAnotherHoldWithoutLosingSuffix() {
        let snippet = SnippetRecord(name: "Test", triggers: [";kz"], content: "Hello")
        var matcher = ExpansionMatcher(snippets: [snippet], groups: [], config: MatchingConfig())
        var queue = HoldReplayQueue<String>()
        var expansions = 0
        for key in [";", "k", "z"] {
            if case .expand(let match) = matcher.handle(.character(key), bundleID: nil) {
                expansions += 1
                queue.begin()
                matcher.didExpand(match: match, insertedCount: 5)
            }
        }
        for key in [";", "k", "z", "!"] { _ = queue.enqueue(key) }
        while let key = queue.next() {
            if case .expand(let match) = matcher.handle(.character(key), bundleID: nil) {
                expansions += 1
                queue.begin()
                matcher.didExpand(match: match, insertedCount: 5)
            }
        }
        #expect(expansions == 2)
        #expect(matcher.buffer.string == "!")
        #expect(queue.state == .idle)
    }

    @Test func wholeWordUndoCountsSeparatorOnce() {
        let word = match(.wholeWord)
        var matcher = ExpansionMatcher()
        let plan = InsertionPlan(match: word, text: "Hello", cursorOffset: nil, mode: .active)
        matcher.didExpand(match: word, insertedCount: plan.text.count)
        #expect(matcher.handle(.backspace, bundleID: nil) ==
                .undo(UndoAction(deleteCount: 6, retype: ";kz.")))
    }
}
