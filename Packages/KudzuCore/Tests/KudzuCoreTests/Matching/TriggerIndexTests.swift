import Foundation
import Testing
@testable import KudzuCore

@Suite("TriggerIndex – firing")
struct TriggerIndexFiringTests {
    // MARK: immediate

    @Test func immediateFiresOnLastTriggerCharacter() throws {
        let s = snip([";sig"])
        let index = makeIndex([s])
        let results = typeAll(";sig", on: index)
        #expect(results.dropLast().allSatisfy { $0 == nil })
        let match = try #require(results.last ?? nil)
        #expect(match.snippetID == s.id)
        #expect(match.trigger == ";sig")
        #expect(match.typedTrigger == ";sig")
        #expect(match.deleteCount == 4)
        #expect(match.separator == nil)
        #expect(match.fireMode == .immediate)
    }

    @Test func immediateFiresMidWordAfterOtherText() throws {
        let index = makeIndex([snip([";sig"])])
        let match = try #require(type("hello;sig", on: index))
        #expect(match.deleteCount == 4)
    }

    @Test func triggerAtVeryStartOfBuffer() throws {
        let index = makeIndex([snip(["ab"])])
        let match = try #require(type("ab", on: index))
        #expect(match.typedTrigger == "ab")
    }

    @Test func immediateDoesNotFireOnPartialTrigger() {
        let index = makeIndex([snip([";sig"])])
        #expect(type(";si", on: index) == nil)
        #expect(type("sig", on: index) == nil)
    }

    @Test func immediateDoesNotFireOnLaterKeystroke() {
        let index = makeIndex([snip([";sig"])])
        // Trigger is in the buffer but the last keystroke is something else.
        #expect(type(";sigx", on: index) == nil)
    }

    @Test func multipleTriggersPerSnippetAllWork() throws {
        let s = snip([";sig", ";regards", "brb"])
        let index = makeIndex([s])
        for trigger in [";sig", ";regards", "brb"] {
            let match = try #require(type(trigger, on: index))
            #expect(match.snippetID == s.id)
            #expect(match.trigger == trigger)
        }
    }

    @Test func emptyAndMissingTriggersAreSkipped() {
        let index = makeIndex([snip([]), snip([""]), snip(["", ";ok"])])
        #expect(index.triggerCount == 1)
        #expect(type(";ok", on: index) != nil)
    }

    @Test func backspaceInMiddleOfTypingTrigger() throws {
        let index = makeIndex([snip([";sig"])])
        var buffer = KeyBuffer()
        func press(_ input: KeyInput) -> TriggerMatch? {
            buffer.apply(input)
            return index.match(buffer: buffer, lastInput: input, bundleID: nil)
        }
        #expect(press(.character(";")) == nil)
        #expect(press(.character("s")) == nil)
        #expect(press(.character("x")) == nil)
        #expect(press(.backspace) == nil)
        #expect(press(.character("i")) == nil)
        #expect(press(.character("g")) != nil)
    }

    @Test func backspaceAfterCompletionDoesNotRefire() {
        let index = makeIndex([snip([";sig"])])
        var buffer = KeyBuffer()
        for ch in ";sig" { buffer.apply(.character(String(ch))) }
        buffer.apply(.backspace)
        #expect(index.match(buffer: buffer, lastInput: .backspace, bundleID: nil) == nil)
        // Re-typing the final character completes it again.
        buffer.apply(.character("g"))
        #expect(index.match(buffer: buffer, lastInput: .character("g"), bundleID: nil) != nil)
    }

    @Test func resetMidTriggerPreventsMatch() {
        let index = makeIndex([snip([";sig"])])
        var buffer = KeyBuffer()
        for ch in ";s" { buffer.apply(.character(String(ch))) }
        buffer.apply(.reset)
        for ch in "ig" { buffer.apply(.character(String(ch))) }
        #expect(index.match(buffer: buffer, lastInput: .character("g"), bundleID: nil) == nil)
    }

    @Test func nonCharacterLastInputNeverMatches() {
        let index = makeIndex([snip([";sig"])])
        var buffer = KeyBuffer()
        for ch in ";sig" { buffer.apply(.character(String(ch))) }
        #expect(index.match(buffer: buffer, lastInput: .reset, bundleID: nil) == nil)
        #expect(index.match(buffer: buffer, lastInput: .backspace, bundleID: nil) == nil)
    }

    @Test func emptyBufferNeverMatches() {
        let index = makeIndex([snip([";sig"])])
        #expect(index.match(buffer: KeyBuffer(), lastInput: .character("x"), bundleID: nil) == nil)
    }

    @Test func triggersContainingSpaces() throws {
        let index = makeIndex([snip(["my address"])])
        let match = try #require(type("my address", on: index))
        #expect(match.deleteCount == 10)
        #expect(type("my  address", on: index) == nil)
    }

    @Test func emojiTriggers() throws {
        let index = makeIndex([snip(["👍🏽", ":)"]), snip(["🇺🇸"])])
        let thumbs = try #require(type("ok 👍🏽", on: index))
        #expect(thumbs.deleteCount == 1)
        #expect(thumbs.trigger == "👍🏽")
        let flag = try #require(type("🇺🇸", on: index))
        #expect(flag.deleteCount == 1)
        // A different skin tone must not match.
        #expect(type("👍🏿", on: index) == nil)
    }

    @Test func multiScalarInputCharactersMatch() throws {
        let index = makeIndex([snip(["caf\u{E9}"])])
        var buffer = KeyBuffer()
        for piece in ["c", "a", "f", "e\u{301}"] { buffer.apply(.character(piece)) }
        // "e" + combining acute is canonically equivalent to the precomposed trigger.
        let match = index.match(buffer: buffer, lastInput: .character("e\u{301}"), bundleID: nil)
        #expect(match != nil)
        #expect(match?.deleteCount == 4)
    }

    // MARK: whole word

    @Test func wholeWordFiresOnSeparatorAndKeepsItOut() throws {
        let s = snip(["brb"], fire: .wholeWord)
        let index = makeIndex([s])
        let results = typeAll("brb ", on: index)
        #expect(results.dropLast().allSatisfy { $0 == nil })
        let match = try #require(results.last ?? nil)
        #expect(match.snippetID == s.id)
        #expect(match.deleteCount == 3, "deleteCount covers the trigger only, never the separator")
        #expect(match.separator == " ")
        #expect(match.deleteCountIncludingSeparator == 4)
        #expect(match.typedTrigger == "brb")
        #expect(match.fireMode == .wholeWord)
    }

    @Test func wholeWordWithEnterSeparator() throws {
        let index = makeIndex([snip(["brb"], fire: .wholeWord)])
        let match = try #require(type("brb\n", on: index))
        #expect(match.separator == "\n")
        #expect(match.deleteCount == 3)
        let crlf = try #require(type("brb\r\n", on: index))
        #expect(crlf.separator == "\r\n")
    }

    @Test(arguments: [" ", "\t", "\n", ".", ",", "!", "?", ";", ":", ")", "]", "}", "'", "\""])
    func wholeWordDefaultSeparators(separator: String) throws {
        let index = makeIndex([snip(["brb"], fire: .wholeWord)])
        let match = try #require(type("brb" + separator, on: index))
        #expect(match.separator == separator)
        #expect(match.deleteCount == 3)
    }

    @Test(arguments: ["-", "_", "a", "1", "/", "("])
    func wholeWordIgnoresNonSeparators(character: String) {
        let index = makeIndex([snip(["brb"], fire: .wholeWord)])
        #expect(type("brb" + character, on: index) == nil)
    }

    @Test func wholeWordDoesNotFireWithoutTrailingSeparator() {
        let index = makeIndex([snip(["brb"], fire: .wholeWord)])
        #expect(typeAll("brb", on: index).allSatisfy { $0 == nil })
    }

    @Test func wholeWordRequiresBoundaryBeforeTrigger() throws {
        let index = makeIndex([snip(["brb"], fire: .wholeWord)])
        #expect(type("abrb ", on: index) == nil)
        #expect(type("1brb ", on: index) == nil)
        let afterSpace = try #require(type("so brb ", on: index))
        #expect(afterSpace.deleteCount == 3)
        #expect(type("so,brb ", on: index) != nil)
        #expect(type("(brb ", on: index) == nil, "( is not in the default separator set")
        #expect(type("\nbrb ", on: index) != nil)
    }

    @Test func wholeWordAtStartOfBuffer() {
        let index = makeIndex([snip(["brb"], fire: .wholeWord)])
        #expect(type("brb ", on: index) != nil)
    }

    @Test func wholeWordTriggerStartingWithSeparatorCharacter() throws {
        // `;` is a separator, but it belongs to the trigger itself.
        let index = makeIndex([snip([";sig"], fire: .wholeWord)])
        let match = try #require(type(";sig ", on: index))
        #expect(match.deleteCount == 4)
        #expect(match.separator == " ")
        #expect(type("hello ;sig ", on: index) != nil)
        #expect(type("hello;sig ", on: index) == nil, "no boundary before the trigger")
    }

    @Test func wholeWordTriggerEndingWithSeparatorNeedsAnotherSeparator() throws {
        let index = makeIndex([snip(["brb."], fire: .wholeWord)])
        // Typing the "." completes the trigger text but the "." itself is the just-typed separator,
        // which leaves "brb" – not the trigger – before it.
        #expect(type("brb.", on: index) == nil)
        let match = try #require(type("brb. ", on: index))
        #expect(match.typedTrigger == "brb.")
        #expect(match.deleteCount == 4)
    }

    @Test func wholeWordTriggerWithSpaces() throws {
        let index = makeIndex([snip(["my address"], fire: .wholeWord)])
        let match = try #require(type("hi my address!", on: index))
        #expect(match.deleteCount == 10)
        #expect(match.separator == "!")
    }

    @Test func overlappingWholeWordTriggersBothFire() throws {
        let a = snip([";a"], fire: .wholeWord)
        let ab = snip([";ab"], fire: .wholeWord)
        let index = makeIndex([a, ab])
        let first = try #require(type(";a ", on: index))
        #expect(first.snippetID == a.id)
        let second = try #require(type(";ab ", on: index))
        #expect(second.snippetID == ab.id)
        // Typing `;a` on the way to `;ab` must not fire anything.
        #expect(typeAll(";ab", on: index).allSatisfy { $0 == nil })
    }

    @Test func immediateShadowsLongerTrigger() throws {
        let a = snip([";a"])
        let ab = snip([";ab"])
        let index = makeIndex([a, ab])
        // `;a` fires the moment it is typed, so `;ab` is unreachable.
        let match = try #require(firstMatch(";ab", on: index))
        #expect(match.snippetID == a.id)
    }

    @Test func immediateAndWholeWordCoexist() throws {
        let imm = snip([";i"])
        let ww = snip(["btw"], fire: .wholeWord)
        let index = makeIndex([imm, ww])
        #expect(try #require(type(";i", on: index)).snippetID == imm.id)
        #expect(try #require(type("btw ", on: index)).snippetID == ww.id)
        #expect(type("btw", on: index) == nil)
    }

    // MARK: case

    @Test func propagateMatchesCaseInsensitively() throws {
        let index = makeIndex([snip([";sig"])])
        for typed in [";sig", ";Sig", ";SIG", ";sIg"] {
            let match = try #require(type(typed, on: index), "\(typed) should match")
            #expect(match.trigger == ";sig")
            #expect(match.typedTrigger == typed)
        }
    }

    @Test func propagateReportsCaseStyle() throws {
        let index = makeIndex([snip([";sig"])])
        #expect(try #require(type(";sig", on: index)).caseStyle == .asIs)
        #expect(try #require(type(";Sig", on: index)).caseStyle == .capitalized)
        #expect(try #require(type(";SIG", on: index)).caseStyle == .upper)
    }

    @Test func exactIsCaseSensitiveAndNeverPropagates() throws {
        let index = makeIndex([snip([";Sig"], case: .exact)])
        let match = try #require(type(";Sig", on: index))
        #expect(match.caseStyle == .asIs)
        #expect(type(";sig", on: index) == nil)
        #expect(type(";SIG", on: index) == nil)
    }

    @Test func propagateWithUnicodeLetters() throws {
        let index = makeIndex([snip([";éclair"])])
        let upper = try #require(type(";ÉCLAIR", on: index))
        #expect(upper.caseStyle == .upper)
        let cap = try #require(type(";Éclair", on: index))
        #expect(cap.caseStyle == .capitalized)
    }

    @Test func wholeWordPropagate() throws {
        let index = makeIndex([snip(["brb"], fire: .wholeWord)])
        let match = try #require(type("BRB ", on: index))
        #expect(match.caseStyle == .upper)
        #expect(match.typedTrigger == "BRB")
    }

    @Test func exactAndPropagateCoexistOnSameLastCharacter() throws {
        let exact = snip(["Ab"], case: .exact)
        let prop = snip(["xb"])
        let index = makeIndex([exact, prop])
        #expect(try #require(type("Ab", on: index)).snippetID == exact.id)
        #expect(try #require(type("XB", on: index)).snippetID == prop.id)
        #expect(type("ab", on: index) == nil)
    }

    // MARK: ranking

    @Test func longestTriggerWins() throws {
        // Both triggers end the buffer on the same keystroke; the longer one must win.
        let tail = snip(["ail"])
        let full = snip(["gmail"])
        let match = try #require(type("gmail", on: makeIndex([tail, full])))
        #expect(match.snippetID == full.id)
        #expect(match.deleteCount == 5)
        let reversed = try #require(type("gmail", on: makeIndex([full, tail])))
        #expect(reversed.snippetID == full.id)
    }

    @Test func tieBreaksOnMostRecentlyUpdated() throws {
        let old = snip([";x"], updated: 100)
        let new = snip([";x"], updated: 200)
        let index = makeIndex([old, new])
        #expect(try #require(type(";x", on: index)).snippetID == new.id)
        let reversed = makeIndex([new, old])
        #expect(try #require(type(";x", on: reversed)).snippetID == new.id)
    }

    @Test func tieWithSameTimestampIsDeterministic() throws {
        let a = snip([";x"], updated: 5)
        let b = snip([";x"], updated: 5)
        let first = try #require(type(";x", on: makeIndex([a, b]))).snippetID
        let again = try #require(type(";x", on: makeIndex([a, b]))).snippetID
        #expect(first == again)
    }

    @Test func caseInsensitiveDuplicateTieBreaksOnRecency() throws {
        let old = snip([";X"], updated: 1)
        let new = snip([";x"], updated: 2)
        #expect(try #require(type(";x", on: makeIndex([old, new]))).snippetID == new.id)
    }

    @Test func longerWholeWordBeatsShorterImmediateEndingSameBuffer() throws {
        // Both candidates are valid on the same keystroke: immediate trigger "o " (ends with the space)
        // and whole-word "hello". Longest trigger wins.
        let imm = snip(["o "])
        let ww = snip(["hello"], fire: .wholeWord)
        let match = try #require(type("hello ", on: makeIndex([imm, ww])))
        #expect(match.snippetID == ww.id)
    }
}

@Suite("TriggerIndex – enablement, rules, insertion")
struct TriggerIndexRuleTests {
    // MARK: enabled

    @Test func disabledSnippetNeverFires() {
        let index = makeIndex([snip([";x"], enabled: false)])
        #expect(index.triggerCount == 0)
        #expect(type(";x", on: index) == nil)
    }

    @Test func disabledParentGroupDisablesSnippet() {
        let g = group(enabled: false)
        let index = makeIndex([snip([";x"], group: g.id)], groups: [g])
        #expect(type(";x", on: index) == nil)
    }

    @Test func disabledGrandparentGroupDisablesSnippet() {
        let top = group("top", enabled: false)
        let mid = group("mid", parent: top.id)
        let leaf = group("leaf", parent: mid.id)
        let index = makeIndex([snip([";x"], group: leaf.id)], groups: [leaf, mid, top])
        #expect(type(";x", on: index) == nil)
    }

    @Test func enabledChainFires() {
        let top = group("top")
        let leaf = group("leaf", parent: top.id)
        let index = makeIndex([snip([";x"], group: leaf.id)], groups: [leaf, top])
        #expect(type(";x", on: index) != nil)
    }

    @Test func disabledLeafGroupOnlyAffectsItsOwnSnippets() {
        let on = group("on")
        let off = group("off", enabled: false)
        let index = makeIndex(
            [snip([";a"], group: on.id), snip([";b"], group: off.id)],
            groups: [on, off]
        )
        #expect(type(";a", on: index) != nil)
        #expect(type(";b", on: index) == nil)
    }

    @Test func groupCycleDoesNotHangAndStillHonoursEnablement() {
        let aID = UUID(), bID = UUID()
        let a = group("a", parent: bID, enabled: true, id: aID)
        let b = group("b", parent: aID, enabled: true, id: bID)
        let index = makeIndex([snip([";x"], group: aID)], groups: [a, b])
        #expect(type(";x", on: index) != nil)

        let disabledB = group("b", parent: aID, enabled: false, id: bID)
        let index2 = makeIndex([snip([";x"], group: aID)], groups: [a, disabledB])
        #expect(type(";x", on: index2) == nil, "a disabled group inside the cycle still disables")
    }

    @Test func groupSelfParentCycle() {
        let id = UUID()
        let g = group("self", parent: id, id: id)
        let index = makeIndex([snip([";x"], group: id)], groups: [g])
        #expect(type(";x", on: index) != nil)
    }

    @Test func unknownGroupIDTreatedAsUngrouped() {
        let index = makeIndex([snip([";x"], group: UUID())])
        #expect(type(";x", on: index) != nil)
    }

    // MARK: app rules

    @Test func allowRuleMatchesOnlyListedApps() {
        let rule = AppRule(mode: .allow, bundleIDs: ["com.apple.Notes"])
        let index = makeIndex([snip([";x"], rule: rule)])
        #expect(type(";x", on: index, bundleID: "com.apple.Notes") != nil)
        #expect(type(";x", on: index, bundleID: "com.apple.Safari") == nil)
        #expect(type(";x", on: index, bundleID: nil) == nil, "unknown frontmost app cannot satisfy an allow-list")
    }

    @Test func denyRuleMatchesEverywhereExceptListedApps() {
        let rule = AppRule(mode: .deny, bundleIDs: ["com.apple.Terminal"])
        let index = makeIndex([snip([";x"], rule: rule)])
        #expect(type(";x", on: index, bundleID: "com.apple.Terminal") == nil)
        #expect(type(";x", on: index, bundleID: "com.apple.Notes") != nil)
        #expect(type(";x", on: index, bundleID: nil) != nil)
    }

    @Test func snippetRuleOverridesGroupRule() {
        let g = group(rule: AppRule(mode: .allow, bundleIDs: ["com.a"]))
        let own = snip([";own"], group: g.id, rule: AppRule(mode: .allow, bundleIDs: ["com.b"]))
        let inherited = snip([";inh"], group: g.id)
        let index = makeIndex([own, inherited], groups: [g])
        #expect(type(";own", on: index, bundleID: "com.b") != nil)
        #expect(type(";own", on: index, bundleID: "com.a") == nil)
        #expect(type(";inh", on: index, bundleID: "com.a") != nil)
        #expect(type(";inh", on: index, bundleID: "com.b") == nil)
    }

    @Test func nearestAncestorGroupRuleWins() {
        let top = group("top", rule: AppRule(mode: .allow, bundleIDs: ["com.top"]))
        let mid = group("mid", parent: top.id, rule: AppRule(mode: .allow, bundleIDs: ["com.mid"]))
        let leaf = group("leaf", parent: mid.id)
        let index = makeIndex([snip([";x"], group: leaf.id)], groups: [top, mid, leaf])
        #expect(type(";x", on: index, bundleID: "com.mid") != nil)
        #expect(type(";x", on: index, bundleID: "com.top") == nil)
    }

    @Test func ruleInheritedThroughGroupWithoutRule() {
        let top = group("top", rule: AppRule(mode: .deny, bundleIDs: ["com.bad"]))
        let leaf = group("leaf", parent: top.id)
        let index = makeIndex([snip([";x"], group: leaf.id)], groups: [top, leaf])
        #expect(type(";x", on: index, bundleID: "com.bad") == nil)
        #expect(type(";x", on: index, bundleID: "com.ok") != nil)
    }

    @Test func globalDenylistAlwaysApplies() {
        let config = MatchingConfig(deniedBundleIDs: ["com.1password.1password"])
        // Even an explicit allow rule naming the denied app cannot override the global list.
        let rule = AppRule(mode: .allow, bundleIDs: ["com.1password.1password"])
        let index = makeIndex([snip([";x"], rule: rule), snip([";y"])], config: config)
        #expect(type(";x", on: index, bundleID: "com.1password.1password") == nil)
        #expect(type(";y", on: index, bundleID: "com.1password.1password") == nil)
        #expect(type(";y", on: index, bundleID: "com.apple.Notes") != nil)
    }

    @Test func defaultConfigDeniesPasswordManagers() {
        let index = makeIndex([snip([";y"])])
        for id in ["com.1password.1password", "com.bitwarden.desktop", "com.apple.keychainaccess", "com.apple.loginwindow"] {
            #expect(type(";y", on: index, bundleID: id) == nil, "\(id) should be denied by default")
        }
    }

    @Test func ruleExcludedSnippetDoesNotBlockOtherCandidate() throws {
        // The longer trigger is not allowed in this app, so the shorter one must still fire.
        let blocked = snip(["xyz"], rule: AppRule(mode: .allow, bundleIDs: ["com.only"]))
        let ok = snip(["yz"])
        let index = makeIndex([blocked, ok])
        let match = try #require(type("xyz", on: index, bundleID: "com.other"))
        #expect(match.snippetID == ok.id)
    }

    // MARK: insertion

    @Test func insertionFallsBackToConfigDefault() throws {
        let index = makeIndex([snip([";x"])], config: MatchingConfig(defaultInsertion: .paste))
        #expect(try #require(type(";x", on: index)).insertion == .paste)
    }

    @Test func insertionPrecedenceSnippetThenGroupThenConfig() throws {
        let top = group("top", insertion: .type)
        let leaf = group("leaf", parent: top.id)
        let config = MatchingConfig(defaultInsertion: .paste)
        let index = makeIndex(
            [
                snip([";own"], group: leaf.id, insertion: .automatic),
                snip([";grp"], group: leaf.id),
                snip([";cfg"]),
            ],
            groups: [top, leaf],
            config: config
        )
        #expect(try #require(type(";own", on: index)).insertion == .automatic)
        #expect(try #require(type(";grp", on: index)).insertion == .type)
        #expect(try #require(type(";cfg", on: index)).insertion == .paste)
    }

    @Test func nearestGroupInsertionWins() throws {
        let top = group("top", insertion: .type)
        let leaf = group("leaf", parent: top.id, insertion: .paste)
        let index = makeIndex([snip([";x"], group: leaf.id)], groups: [top, leaf])
        #expect(try #require(type(";x", on: index)).insertion == .paste)
    }

    @Test func ruleInsertionOverrideAppliesWhenAppListed() throws {
        let rule = AppRule(mode: .allow, bundleIDs: ["com.term", "com.notes"], insertion: .paste)
        let index = makeIndex([snip([";x"], insertion: .type, rule: rule)])
        #expect(try #require(type(";x", on: index, bundleID: "com.term")).insertion == .paste)
        #expect(try #require(type(";x", on: index, bundleID: "com.notes")).insertion == .paste)
    }

    @Test func ruleInsertionOverrideInheritedFromGroupRule() throws {
        let g = group(rule: AppRule(mode: .allow, bundleIDs: ["com.term"], insertion: .paste))
        let index = makeIndex([snip([";x"], group: g.id)], groups: [g])
        #expect(try #require(type(";x", on: index, bundleID: "com.term")).insertion == .paste)
    }

    @Test func ruleInsertionOverrideNotAppliedToUnlistedApp() throws {
        // A deny rule with an insertion override: unlisted apps keep the normal insertion.
        let rule = AppRule(mode: .deny, bundleIDs: ["com.term"], insertion: .paste)
        let index = makeIndex([snip([";x"], insertion: .type, rule: rule)])
        #expect(try #require(type(";x", on: index, bundleID: "com.notes")).insertion == .type)
    }

    @Test func ruleWithoutInsertionLeavesInsertionAlone() throws {
        let rule = AppRule(mode: .allow, bundleIDs: ["com.term"])
        let index = makeIndex([snip([";x"], insertion: .type, rule: rule)])
        #expect(try #require(type(";x", on: index, bundleID: "com.term")).insertion == .type)
    }
}

@Suite("TriggerIndex – performance")
struct TriggerIndexPerformanceTests {
    @Test func matchingStaysFastWithFiveThousandSnippets() throws {
        var snippets: [SnippetRecord] = []
        // Many triggers share their last character (digits) – the worst case for a last-char bucket index.
        for i in 0..<5_000 {
            snippets.append(snip([";s\(i)", "tag\(i)x"], fire: i.isMultiple(of: 2) ? .immediate : .wholeWord))
        }
        let index = makeIndex(snippets)
        #expect(index.triggerCount == 10_000)

        let typed = Array("The quick brown fox jumps over the lazy dog ;s42 and ;s4999 tag12x 0123456789 ".map(String.init))
        var buffer = KeyBuffer()
        var matches = 0
        let rounds = 40
        let clock = ContinuousClock()
        let elapsed = clock.measure {
            for _ in 0..<rounds {
                for piece in typed {
                    let input = KeyInput.character(piece)
                    buffer.apply(input)
                    if index.match(buffer: buffer, lastInput: input, bundleID: "com.apple.Notes") != nil {
                        matches += 1
                        buffer.apply(.reset)
                    }
                }
            }
        }
        let keystrokes = rounds * typed.count
        let perKeystroke = elapsed / keystrokes
        #expect(matches > 0)
        // Loose bound (unoptimised debug build, shared CI machines); real cost is a few microseconds.
        #expect(perKeystroke < .milliseconds(1), "avg \(perKeystroke) per keystroke")
    }

    @Test func buildingLargeIndexIsReasonable() {
        let snippets = (0..<5_000).map { snip([";s\($0)"]) }
        let clock = ContinuousClock()
        let elapsed = clock.measure { _ = makeIndex(snippets) }
        #expect(elapsed < .seconds(5))
    }
}
