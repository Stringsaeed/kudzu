import Foundation
import Testing
import KudzuCore
@testable import KudzuApp

@Test func previewTruncatesBeforeHighlightingAndCapsCodeLines() {
    let head = "alpha " + String(repeating: "a", count: ClipPreviewRules.renderLimit)
    let kept = ClipPreviewRules.textPreview(head, terms: ["alpha", "zzz"])
    #expect(kept.shown.hasPrefix("alpha "))
    #expect(kept.shown.utf16.count == ClipPreviewRules.renderLimit)
    #expect(kept.hiddenCharacters == head.utf16.count - ClipPreviewRules.renderLimit)
    #expect(kept.ranges == [0..<5])

    let tail = String(repeating: "a", count: ClipPreviewRules.renderLimit) + "alpha"
    let cut = ClipPreviewRules.textPreview(tail, terms: ["alpha"])
    #expect(cut.ranges.isEmpty)
    #expect(cut.hiddenCharacters == 5)
    #expect(ClipPreviewRules.moreCharacters(5) == "…and 5 more characters. Copy it to get everything.")
    #expect(ClipPreviewRules.moreCharacters(0) == nil)

    let emoji = String(repeating: "a", count: ClipPreviewRules.renderLimit - 1) + "😀"
    let pair = ClipPreviewRules.prefix(emoji, ClipPreviewRules.renderLimit)
    #expect(pair.shown == String(repeating: "a", count: ClipPreviewRules.renderLimit - 1))
    #expect(pair.hidden == 2)

    let exact = String(repeating: "b", count: ClipPreviewRules.renderLimit)
    #expect(ClipPreviewRules.textPreview(exact, terms: ["b"]).hiddenCharacters == 0)

    let code = ClipPreviewRules.codePreview(String(repeating: "x\n", count: ClipPreviewRules.codeLineLimit))
    #expect(code.lines.count == ClipPreviewRules.codeLineLimit)
    #expect(code.hiddenLines == 1)
    #expect(code.lines.last == "x")
    #expect(ClipPreviewRules.moreLines(1) == "…and 1 more lines.")
    #expect(ClipPreviewRules.codePreview("a\nb").lines == ["a", "b"])
}

@Test func rowListTextTruncatesInUTF16CodeUnits() {
    let long = String(repeating: "a", count: ClipPreviewRules.rowTextLimit) + "tail"
    #expect(ClipPreviewRules.rowText(long).utf16.count == ClipPreviewRules.rowTextLimit)
    #expect(ClipPreviewRules.rowText("short") == "short")
}

@Test func excerptCompleteUsesJavaScriptUTF16Length() {
    let family = "👨‍👩‍👧"
    let textLen = family.utf16.count
    #expect(textLen > family.count)
    #expect(ClipPreviewRules.excerptIsComplete(textLen: textLen, excerpt: family))
    #expect(!ClipPreviewRules.excerptIsComplete(textLen: textLen + 1, excerpt: family))
}

@Test func historyPinnedSectionDefersWhileSearching() {
    #expect(ClipPreviewRules.pinnedFirst(picker: false, query: "", filter: "all"))
    #expect(!ClipPreviewRules.pinnedFirst(picker: false, query: "needle", filter: "all"))
    #expect(!ClipPreviewRules.pinnedFirst(picker: false, query: " ", filter: "all"))
    #expect(!ClipPreviewRules.pinnedFirst(picker: false, query: "", filter: "pinned"))
    #expect(!ClipPreviewRules.pinnedFirst(picker: true, query: "", filter: "all"))
}

@Test func copyTimelineAnchorPrefersCopiedAtWhenTimingsAreEmpty() {
    let copied = Date(timeIntervalSince1970: 1_900_000_000)
    let first = Date(timeIntervalSince1970: 1_800_000_000)
    #expect(ClipPreviewRules.copyTimelineAnchor(timings: [first], copiedAt: copied) == first)
    #expect(ClipPreviewRules.copyTimelineAnchor(timings: [], copiedAt: copied) == copied)
}
