import Foundation
import Testing
@testable import KudzuCore

@Suite("ExpansionMatcher")
struct ExpansionMatcherTests {
    private func type(_ text: String, into matcher: inout ExpansionMatcher, bundleID: String? = nil) -> MatcherEvent? {
        var last: MatcherEvent?
        for ch in text { last = matcher.handle(.character(String(ch)), bundleID: bundleID) }
        return last
    }

    @Test func expandsOnTrigger() {
        let s = snip([";sig"])
        var matcher = ExpansionMatcher(snippets: [s], groups: [], config: MatchingConfig())
        let event = type(";sig", into: &matcher)
        guard case .expand(let match)? = event else {
            Issue.record("expected expand"); return
        }
        #expect(match.snippetID == s.id)
    }

    @Test func noEventWhenNothingMatches() {
        var matcher = ExpansionMatcher(snippets: [snip([";sig"])], groups: [], config: MatchingConfig())
        #expect(type("hello", into: &matcher) == nil)
        #expect(matcher.handle(.backspace, bundleID: nil) == nil)
        #expect(matcher.handle(.reset, bundleID: nil) == nil)
    }

    @Test func backspaceMidTriggerThroughFacade() {
        var matcher = ExpansionMatcher(snippets: [snip([";sig"])], groups: [], config: MatchingConfig())
        _ = type(";sx", into: &matcher)
        _ = matcher.handle(.backspace, bundleID: nil)
        #expect(type("ig", into: &matcher) != nil)
    }

    @Test func resetMidTriggerThroughFacade() {
        var matcher = ExpansionMatcher(snippets: [snip([";sig"])], groups: [], config: MatchingConfig())
        _ = type(";s", into: &matcher)
        _ = matcher.handle(.reset, bundleID: nil)
        #expect(type("ig", into: &matcher) == nil)
    }

    @Test func backspaceAfterExpansionUndoes() {
        var matcher = ExpansionMatcher(snippets: [snip([";sig"])], groups: [], config: MatchingConfig())
        let event = type(";Sig", into: &matcher)
        guard case .expand(let match)? = event else {
            Issue.record("expected expand"); return
        }
        matcher.didExpand(match: match, insertedCount: 12)
        let undo = matcher.handle(.backspace, bundleID: nil)
        #expect(undo == .undo(UndoAction(deleteCount: 12, retype: ";Sig")))
        // Undo is one-shot and the restored trigger must not immediately re-expand.
        #expect(matcher.handle(.backspace, bundleID: nil) == nil)
    }

    @Test func wholeWordUndoRestoresSeparator() {
        var matcher = ExpansionMatcher(snippets: [snip(["brb"], fire: .wholeWord)], groups: [], config: MatchingConfig())
        let event = type("brb ", into: &matcher)
        guard case .expand(let match)? = event else {
            Issue.record("expected expand"); return
        }
        #expect(match.separator == " ")
        matcher.didExpand(match: match, insertedCount: 15)
        #expect(matcher.handle(.backspace, bundleID: nil) == .undo(UndoAction(deleteCount: 16, retype: "brb ")))
    }

    @Test func typingAfterExpansionCancelsUndo() {
        var matcher = ExpansionMatcher(snippets: [snip([";sig"])], groups: [], config: MatchingConfig())
        let event = type(";sig", into: &matcher)
        guard case .expand(let match)? = event else {
            Issue.record("expected expand"); return
        }
        matcher.didExpand(match: match, insertedCount: 5)
        _ = matcher.handle(.character("x"), bundleID: nil)
        #expect(matcher.handle(.backspace, bundleID: nil) == nil)
    }

    @Test func expansionResetsBufferSoOldTextCannotRetrigger() {
        var matcher = ExpansionMatcher(snippets: [snip([";a"]), snip(["ab"])], groups: [], config: MatchingConfig())
        let event = type(";a", into: &matcher)
        guard case .expand(let match)? = event else {
            Issue.record("expected expand"); return
        }
        matcher.didExpand(match: match, insertedCount: 3)
        #expect(matcher.buffer.isEmpty)
        // "b" alone must not combine with the consumed ";a" to form "ab".
        #expect(matcher.handle(.character("b"), bundleID: nil) == nil)
    }

    @Test func undoAfterExpansionThenNormalTypingContinues() {
        var matcher = ExpansionMatcher(snippets: [snip([";sig"])], groups: [], config: MatchingConfig())
        let event = type(";sig", into: &matcher)
        guard case .expand(let match)? = event else {
            Issue.record("expected expand"); return
        }
        matcher.didExpand(match: match, insertedCount: 3)
        _ = matcher.handle(.backspace, bundleID: nil)
        // The app retyped ";sig"; the user backspaces once and types "g" again → expands again.
        _ = matcher.handle(.backspace, bundleID: nil)
        let again = matcher.handle(.character("g"), bundleID: nil)
        guard case .expand? = again else {
            Issue.record("expected expand"); return
        }
    }

    @Test func bundleIDIsRespectedPerCall() {
        let rule = AppRule(mode: .allow, bundleIDs: ["com.ok"])
        var matcher = ExpansionMatcher(snippets: [snip([";x"], rule: rule)], groups: [], config: MatchingConfig())
        #expect(type(";x", into: &matcher, bundleID: "com.no") == nil)
        _ = matcher.handle(.reset, bundleID: nil)
        #expect(type(";x", into: &matcher, bundleID: "com.ok") != nil)
    }

    @Test func reloadSwapsIndexAndClearsUndo() {
        var matcher = ExpansionMatcher(snippets: [snip([";old"])], groups: [], config: MatchingConfig())
        let event = type(";old", into: &matcher)
        guard case .expand(let match)? = event else {
            Issue.record("expected expand"); return
        }
        matcher.didExpand(match: match, insertedCount: 3)
        matcher.reload(snippets: [snip([";new"])], groups: [], config: MatchingConfig())
        #expect(matcher.handle(.backspace, bundleID: nil) == nil, "undo is cleared on reload")
        _ = matcher.handle(.reset, bundleID: nil)
        #expect(type(";old", into: &matcher) == nil)
        #expect(type(";new", into: &matcher) != nil)
    }

    @Test func emptyMatcherIsInert() {
        var matcher = ExpansionMatcher()
        #expect(type("anything", into: &matcher) == nil)
    }
}
