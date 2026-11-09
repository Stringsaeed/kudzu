import Foundation
import Testing
@testable import KudzuCore

@Suite("UndoTracker")
struct UndoTrackerTests {
    private func immediateMatch(typed: String = ";sig") -> TriggerMatch {
        TriggerMatch(
            snippetID: UUID(), trigger: ";sig", typedTrigger: typed, deleteCount: typed.count,
            separator: nil, insertion: .type, caseStyle: .asIs, fireMode: .immediate
        )
    }

    private func wholeWordMatch(typed: String = "brb", separator: String = " ") -> TriggerMatch {
        TriggerMatch(
            snippetID: UUID(), trigger: "brb", typedTrigger: typed, deleteCount: typed.count,
            separator: separator, insertion: .type, caseStyle: .asIs, fireMode: .wholeWord
        )
    }

    func backspaceRightAfterImmediateExpansionUndoes() {
        var tracker = UndoTracker()
        tracker.recordExpansion(of: immediateMatch(typed: ";Sig"), insertedCount: 12)
        let action = tracker.consume(.backspace)
        #expect(action == UndoAction(deleteCount: 12, retype: ";Sig"))
        #expect(!tracker.hasPending)
    }

    func wholeWordUndoAlsoRemovesAndRestoresSeparator() {
        var tracker = UndoTracker()
        tracker.recordExpansion(of: wholeWordMatch(typed: "BRB", separator: "\n"), insertedCount: 20)
        // The re-emitted separator sits after the inserted text, so it is deleted too and retyped.
        #expect(tracker.consume(.backspace) == UndoAction(deleteCount: 21, retype: "BRB\n"))
    }

    @Test func anyOtherInputClearsPendingUndo() {
        for other: KeyInput in [.character("x"), .reset] {
            var tracker = UndoTracker()
            tracker.recordExpansion(of: immediateMatch(), insertedCount: 5)
            #expect(tracker.consume(other) == nil)
            #expect(!tracker.hasPending)
            #expect(tracker.consume(.backspace) == nil, "undo must not survive an intervening input")
        }
    }

    func undoIsOneShot() {
        var tracker = UndoTracker()
        tracker.recordExpansion(of: immediateMatch(), insertedCount: 5)
        #expect(tracker.consume(.backspace) != nil)
        #expect(tracker.consume(.backspace) == nil)
    }

    func nothingPendingMeansNoUndo() {
        var tracker = UndoTracker()
        #expect(tracker.consume(.backspace) == nil)
    }

    func newExpansionReplacesPendingOne() {
        var tracker = UndoTracker()
        tracker.recordExpansion(of: immediateMatch(typed: ";a"), insertedCount: 1)
        tracker.recordExpansion(of: immediateMatch(typed: ";b"), insertedCount: 2)
        #expect(tracker.consume(.backspace) == UndoAction(deleteCount: 2, retype: ";b"))
    }

    func clearDropsPending() {
        var tracker = UndoTracker()
        tracker.recordExpansion(of: immediateMatch(), insertedCount: 5)
        tracker.clear()
        #expect(tracker.consume(.backspace) == nil)
    }

    func zeroInsertedCountStillRestoresTrigger() {
        var tracker = UndoTracker()
        tracker.recordExpansion(of: immediateMatch(), insertedCount: 0)
        #expect(tracker.consume(.backspace) == UndoAction(deleteCount: 0, retype: ";sig"))
    }
}
