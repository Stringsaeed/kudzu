import Foundation
import Testing
@testable import KudzuCore

@Suite("TriggerConflicts")
struct TriggerConflictsTests {
    @Test func noConflictsForDistinctTriggers() {
        #expect(TriggerConflicts.find(snippets: [snip([";a"]), snip([";b"])], groups: []).isEmpty)
    }

    @Test func duplicateTriggerOnTwoSnippets() throws {
        let a = snip([";sig"]), b = snip(["x", ";sig"])
        let found = TriggerConflicts.find(snippets: [a, b], groups: [])
        let conflict = try #require(found.first)
        #expect(found.count == 1)
        #expect(conflict.kind == .duplicate)
        #expect(Set([conflict.snippetID, conflict.otherSnippetID]) == Set([a.id, b.id]))
        #expect(conflict.trigger == ";sig")
        #expect(conflict.otherTrigger == ";sig")
    }

    func duplicateIsCaseInsensitiveWhenEitherSidePropagates() {
        let prop = snip([";Sig"])
        let exact = snip([";sig"], case: .exact)
        #expect(TriggerConflicts.find(snippets: [prop, exact], groups: []).count == 1)
        let prop2 = snip([";SIG"])
        #expect(TriggerConflicts.find(snippets: [prop, prop2], groups: []).count == 1)
    }

    func exactTriggersDifferingByCaseDoNotConflict() {
        let a = snip([";Sig"], case: .exact), b = snip([";sig"], case: .exact)
        #expect(TriggerConflicts.find(snippets: [a, b], groups: []).isEmpty)
    }

    func exactTriggersWithSameCaseConflict() {
        let a = snip([";sig"], case: .exact), b = snip([";sig"], case: .exact)
        #expect(TriggerConflicts.find(snippets: [a, b], groups: []).count == 1)
    }

    func sameSnippetRepeatingATriggerIsNotAConflict() {
        #expect(TriggerConflicts.find(snippets: [snip([";a", ";A"])], groups: []).isEmpty)
    }

    @Test func disabledSnippetsAreIgnored() {
        let a = snip([";sig"]), b = snip([";sig"], enabled: false)
        #expect(TriggerConflicts.find(snippets: [a, b], groups: []).isEmpty)
        let g = group(enabled: false)
        let c = snip([";sig"], group: g.id)
        #expect(TriggerConflicts.find(snippets: [a, c], groups: [g]).isEmpty)
    }

    @Test func immediatePrefixShadowsLongerTrigger() throws {
        let short = snip([";s"]), long = snip([";sig"])
        let found = TriggerConflicts.find(snippets: [long, short], groups: [])
        let conflict = try #require(found.first)
        #expect(found.count == 1)
        #expect(conflict.kind == .shadowing)
        #expect(conflict.snippetID == short.id, "snippetID is the shadowing snippet")
        #expect(conflict.trigger == ";s")
        #expect(conflict.otherSnippetID == long.id)
        #expect(conflict.otherTrigger == ";sig")
    }

    func shadowingAppliesToWholeWordLongerTrigger() {
        let short = snip([";s"]), long = snip([";sig"], fire: .wholeWord)
        let found = TriggerConflicts.find(snippets: [short, long], groups: [])
        #expect(found.count == 1)
        #expect(found.first?.kind == .shadowing)
    }

    func wholeWordPrefixDoesNotShadow() {
        let short = snip([";s"], fire: .wholeWord), long = snip([";sig"])
        #expect(TriggerConflicts.find(snippets: [short, long], groups: []).isEmpty)
        let bothWW = TriggerConflicts.find(snippets: [short, snip([";sig"], fire: .wholeWord)], groups: [])
        #expect(bothWW.isEmpty)
    }

    func shadowingIsCaseInsensitiveWhenShadowerPropagates() {
        let short = snip([";S"]), long = snip([";sig"], case: .exact)
        #expect(TriggerConflicts.find(snippets: [short, long], groups: []).count == 1)
    }

    func exactShadowerDoesNotShadowPropagatingLongerTriggerWithCasedPrefix() {
        // `;s` (exact) cannot stop `;SIG` from firing: the user can type capitals.
        let short = snip([";s"], case: .exact), long = snip([";sig"])
        #expect(TriggerConflicts.find(snippets: [short, long], groups: []).isEmpty)
    }

    func exactShadowerDoesShadowWhenPrefixHasNoCase() {
        let short = snip([";1"], case: .exact), long = snip([";1x"])
        #expect(TriggerConflicts.find(snippets: [short, long], groups: []).count == 1)
    }

    func exactShadowsExactOnlyWithMatchingCase() {
        let short = snip([";s"], case: .exact)
        #expect(TriggerConflicts.find(snippets: [short, snip([";sig"], case: .exact)], groups: []).count == 1)
        #expect(TriggerConflicts.find(snippets: [short, snip([";Sig"], case: .exact)], groups: []).isEmpty)
    }

    func prefixWithinSameSnippetIsIgnored() {
        #expect(TriggerConflicts.find(snippets: [snip([";s", ";sig"])], groups: []).isEmpty)
    }

    func equalTriggersReportAsDuplicateNotShadowing() {
        let found = TriggerConflicts.find(snippets: [snip([";s"]), snip([";s"])], groups: [])
        #expect(found.map(\.kind) == [.duplicate])
    }

    func disjointAllowListsDoNotConflict() {
        let a = snip([";x"], rule: AppRule(mode: .allow, bundleIDs: ["com.a"]))
        let b = snip([";x"], rule: AppRule(mode: .allow, bundleIDs: ["com.b"]))
        #expect(TriggerConflicts.find(snippets: [a, b], groups: []).isEmpty)
        let c = snip([";x"], rule: AppRule(mode: .allow, bundleIDs: ["com.a", "com.b"]))
        #expect(TriggerConflicts.find(snippets: [a, c], groups: []).count == 1)
    }

    func allowListInsideDenyListDoesNotConflict() {
        let allow = snip([";x"], rule: AppRule(mode: .allow, bundleIDs: ["com.a"]))
        let deny = snip([";x"], rule: AppRule(mode: .deny, bundleIDs: ["com.a"]))
        #expect(TriggerConflicts.find(snippets: [allow, deny], groups: []).isEmpty)
        let denyOther = snip([";x"], rule: AppRule(mode: .deny, bundleIDs: ["com.z"]))
        #expect(TriggerConflicts.find(snippets: [allow, denyOther], groups: []).count == 1)
    }

    func groupRulesAreInheritedForOverlapCheck() {
        let g = group(rule: AppRule(mode: .allow, bundleIDs: ["com.a"]))
        let a = snip([";x"], group: g.id)
        let b = snip([";x"], rule: AppRule(mode: .allow, bundleIDs: ["com.b"]))
        #expect(TriggerConflicts.find(snippets: [a, b], groups: [g]).isEmpty)
    }

    func scalesToManySnippets() {
        let snippets = (0..<3_000).map { snip([";s\($0)"]) }
        let clock = ContinuousClock()
        let elapsed = clock.measure {
            #expect(TriggerConflicts.find(snippets: snippets, groups: []).isEmpty == false)
        }
        #expect(elapsed < .seconds(5))
    }
}
