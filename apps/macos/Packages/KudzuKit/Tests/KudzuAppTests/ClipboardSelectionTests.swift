import Foundation
import Testing
import KudzuCore
@testable import KudzuApp

@Test func clipboardBulkPinDecision() {
    var first = ClipEntry(kind: .text, hash: "a", firstCopiedAt: Date(), copiedAt: Date())
    var second = ClipEntry(kind: .text, hash: "b", firstCopiedAt: Date(), copiedAt: Date())
    #expect(ClipSelectionRules.pinTarget([]) == nil)
    #expect(ClipSelectionRules.pinTarget([first, second]) == true)
    first.pinned = true
    #expect(ClipSelectionRules.pinTarget([first, second]) == true)
    second.pinned = true
    #expect(ClipSelectionRules.pinTarget([first, second]) == false)
}

@Test func clipboardHighlightRangesMergeAndUseUTF16() {
    #expect(Clipboard.matchRanges("😀 Alpha alphabet", terms: ["alpha", "bet"]) == [3..<8, 9..<17])
    #expect(Clipboard.matchRanges("nothing", terms: ["missing"]).isEmpty)
}
