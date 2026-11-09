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

    func immediateDoesNotFireOnPartialTrigger() {
        let index = makeIndex([snip([";sig"])])
        #expect(type(";si", on: index) == nil)
        #expect(type("sig", on: index) == nil)
    }

    func immediateDoesNotFireOnLaterKeystroke() {
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

    func nonCharacterLastInputNeverMatches() {
        let index = makeIndex([snip([";sig"])])
        var buffer = KeyBuffer()
        for ch in ";sig" { buffer.apply(.character(String(ch))) }
        #expect(index.match(buffer: buffer, lastInput: .reset, bundleID: nil) == nil)
        #expect(index.match(buffer: buffer, lastInput: .backspace, bundleID: nil) == nil)
    }

    func emptyBufferNeverMatches() {
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

    func multiScalarInputCharactersMatch() throws {
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

    func propagateMatchesCaseInsensitively() throws {
        let index = makeIndex([snip([";sig"])])
        for typed in [";sig", ";Sig", ";SIG", ";sIg"] {
            let match = try #require(type(typed, on: index), "\(typed) should match")
            #expect(match.trigger == ";sig")
            #expect(match.typedTrigger == typed)
        }
    }

    func propagateReportsCaseStyle() throws {
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
