import Foundation
import Testing
@testable import KudzuCore

@Suite("PaletteRanker")
struct PaletteRankerTests {
    private let now = Date(timeIntervalSince1970: 1_800_000_000)

    private func s(
        _ name: String, _ triggers: [String] = [], _ content: String = "",
        favorite: Bool = false, enabled: Bool = true, group: UUID? = nil
    ) -> SnippetRecord {
        SnippetRecord(name: name, triggers: triggers, content: content, groupID: group,
                      isFavorite: favorite, isEnabled: enabled)
    }

    private func names(_ results: [PaletteMatch], _ snippets: [SnippetRecord]) -> [String] {
        let byID = Dictionary(uniqueKeysWithValues: snippets.map { ($0.id, $0.name) })
        return results.map { byID[$0.snippetID] ?? "?" }
    }

    private func used(_ snippet: SnippetRecord, count: Int, daysAgo: Double?) -> (UUID, SnippetStats) {
        (snippet.id, SnippetStats(snippetID: snippet.id, count: count,
                                  lastUsedAt: daysAgo.map { now.addingTimeInterval(-$0 * 86_400) }))
    }

    // MARK: - Matching

    @Test func fuzzySubsequenceMatchesAcrossFields() {
        let a = s("Email Signature", [";sig"], "Best regards")
        let b = s("Home Address", [";addr"], "1 Main Street")
        let ranker = PaletteRanker(snippets: [a, b], now: now)
        #expect(names(ranker.rank("emsig"), [a, b]) == ["Email Signature"])
        #expect(names(ranker.rank("addr"), [a, b]) == ["Home Address"])
        #expect(names(ranker.rank("main street"), [a, b]) == ["Home Address"])
        #expect(ranker.rank("zzz").isEmpty)
    }

    @Test func gappyNameDoesNotBuryExactContent() {
        let exact = s("Unrelated title", [], "qt")
        let loose = s("Something else", [], "qat")
        let gappy = s("Q" + String(repeating: "x", count: 40) + "t notes", [], "qt")
        let all = [exact, loose, gappy]
        let ranked = names(PaletteRanker(snippets: all, now: now).rank("qt"), all)
        #expect(ranked.firstIndex(of: gappy.name)! < ranked.firstIndex(of: loose.name)!)
    }

    @Test func caseAndDiacriticInsensitive() {
        let a = s("Café Menu", [";café"], "Crème brûlée")
        let ranker = PaletteRanker(snippets: [a], now: now)
        #expect(ranker.rank("CAFE").count == 1)
        #expect(ranker.rank("creme").count == 1)
    }

    @Test func everyTermMustMatch() {
        let a = s("Email Signature", [";sig"], "Best regards")
        let ranker = PaletteRanker(snippets: [a], now: now)
        #expect(ranker.rank("sig regards").count == 1)
        #expect(ranker.rank("sig nope").isEmpty)
    }

    @Test func contentDoesNotMatchScatteredLetters() {
        // Any long text contains "abc" as a far-flung subsequence; that must not count as a hit.
        let a = s("Lorem", [";l"], "a lot of text, then much later b, and finally a c")
        let ranker = PaletteRanker(snippets: [a], now: now)
        #expect(ranker.rank("abc").isEmpty)
    }

    @Test func disabledSnippetsAndDisabledGroupsAreExcluded() {
        let off = GroupRecord(name: "Off", isEnabled: false)
        let child = GroupRecord(name: "Child", parentID: off.id)
        let a = s("Alpha", [";a"])
        let b = s("Alpine", [";al"], enabled: false)
        let c = s("Alps", [";alps"], group: child.id)
        let ranker = PaletteRanker(snippets: [a, b, c], groups: [off, child], now: now)
        #expect(names(ranker.rank("al"), [a, b, c]) == ["Alpha"])
        #expect(names(ranker.rank(""), [a, b, c]) == ["Alpha"])
    }

    // MARK: - Ranking

    @Test func exactTriggerOutranksPrefixTriggerOutranksName() {
        let exact = s("Zebra", [";sig"])
        let prefix = s("Yak", [";sigmoid"])
        let name = s("Signature block", [";x"])
        let all = [name, prefix, exact]
        let ranker = PaletteRanker(snippets: all, now: now)
        #expect(names(ranker.rank(";sig"), all) == ["Zebra", "Yak"])
        // Typing the trigger without its prefix character counts too.
        #expect(names(ranker.rank("sig"), all) == ["Zebra", "Yak", "Signature block"])
    }

    @Test func nameOutranksTriggerOutranksContent() {
        let byName = s("Meeting notes", [";x1"], "nothing")
        let byTrigger = s("Other", [";meeting"], "nothing")
        let byContent = s("Third", [";x3"], "see you at the meeting")
        let all = [byContent, byTrigger, byName]
        let ranker = PaletteRanker(snippets: all, now: now)
        // "eeting" is not a trigger prefix, so this compares plain field weights.
        #expect(names(ranker.rank("eeting"), all) == ["Meeting notes", "Other", "Third"])
    }

    @Test func wordBoundaryAndConsecutiveMatchesWin() {
        let boundary = s("Git Status", [";1"])
        let scattered = s("Big Statistics", [";2"])
        let all = [scattered, boundary]
        let ranker = PaletteRanker(snippets: all, now: now)
        #expect(names(ranker.rank("gs"), all).first == "Git Status")

        let consecutive = s("Address book", [";3"])
        let spread = s("A dark dress", [";4"])
        let more = [spread, consecutive]
        #expect(names(PaletteRanker(snippets: more, now: now).rank("addr"), more).first == "Address book")
    }

    @Test func prefixOfNameBeatsMiddleOfName() {
        let prefix = s("Report weekly", [";1"])
        let middle = s("Weekly report", [";2"])
        let all = [middle, prefix]
        #expect(names(PaletteRanker(snippets: all, now: now).rank("rep"), all).first == "Report weekly")
    }

    @Test func usageBoostsEqualMatches() {
        let a = s("Reply thanks", [";1"])
        let b = s("Reply sorry", [";2"])
        let c = s("Reply later", [";3"])
        let all = [a, b, c]
        let stats = Dictionary(uniqueKeysWithValues: [
            used(b, count: 3, daysAgo: 0),
            used(c, count: 50, daysAgo: 200),
        ])
        let ranker = PaletteRanker(snippets: all, stats: stats, now: now)
        #expect(names(ranker.rank("reply"), all) == ["Reply later", "Reply sorry", "Reply thanks"])
    }

    @Test func usageDoesNotDrownAMuchBetterMatch() {
        let good = s("Invoice", [";1"])
        let weak = s("Inline video", [";2"])
        let all = [weak, good]
        let stats = Dictionary(uniqueKeysWithValues: [used(weak, count: 20, daysAgo: 1)])
        let ranker = PaletteRanker(snippets: all, stats: stats, now: now)
        #expect(names(ranker.rank("invoice"), all) == ["Invoice"])
        #expect(names(ranker.rank("invo"), all).first == "Invoice")
    }

    @Test func tiesBreakByNameThenID() {
        let b = s("Beta", [";b"]), a = s("Alpha", [";a"]), a2 = s("Alpha", [";a2"])
        let all = [b, a2, a]
        let ranker = PaletteRanker(snippets: all, now: now)
        let first = ranker.rank("").map(\.snippetID)
        let second = PaletteRanker(snippets: all.reversed(), now: now).rank("").map(\.snippetID)
        #expect(first == second)
        #expect(names(ranker.rank(""), all) == ["Alpha", "Alpha", "Beta"])
    }

    @Test func emptyQueryPutsFavoritesFirstThenUsage() {
        let fav = s("Zulu", favorite: true)
        let recent = s("Yankee")
        let frequentOld = s("X-ray")
        let never = s("Alpha")
        let all = [never, frequentOld, recent, fav]
        let stats = Dictionary(uniqueKeysWithValues: [
            used(recent, count: 2, daysAgo: 0),
            used(frequentOld, count: 4, daysAgo: 300),
        ])
        let ranker = PaletteRanker(snippets: all, stats: stats, now: now)
        #expect(names(ranker.rank("   "), all) == ["Zulu", "Yankee", "X-ray", "Alpha"])
    }

    @Test func limitTruncatesResults() {
        let all = (0..<10).map { s("Item \($0)", [";i\($0)"]) }
        let ranker = PaletteRanker(snippets: all, now: now)
        #expect(ranker.rank("item", limit: 3).count == 3)
        #expect(ranker.rank("", limit: 4).count == 4)
        #expect(ranker.count == 10)
    }

    // MARK: - Highlights

    @Test func nameHighlightsAreCharacterOffsets() throws {
        let a = s("Email Signature", [";x"])
        let match = try #require(PaletteRanker(snippets: [a], now: now).rank("sig").first)
        #expect(match.field == .name)
        #expect(match.nameHighlights == [6, 7, 8])
    }

    @Test func highlightsSurviveFoldingAndCombiningMarks() throws {
        // "e\u{301}" is one Character made of two scalars; offsets count Characters.
        let a = s("Cafe\u{301} Noir", [";x"])
        let match = try #require(PaletteRanker(snippets: [a], now: now).rank("cafen").first)
        #expect(match.nameHighlights == [0, 1, 2, 3, 5])

        let b = s("👋 Hello", [";y"])
        let hello = try #require(PaletteRanker(snippets: [b], now: now).rank("hello").first)
        #expect(hello.nameHighlights == [2, 3, 4, 5, 6])
    }

    @Test func highlightsOnlyForTermsMatchedInName() throws {
        let a = s("Signature", [";zz"], "best regards")
        let match = try #require(PaletteRanker(snippets: [a], now: now).rank("sig regards").first)
        #expect(match.nameHighlights == [0, 1, 2])
    }

    @Test func triggerAndContentMatchesHaveNoNameHighlights() throws {
        let a = s("Other", [";meeting"], "nothing")
        let match = try #require(PaletteRanker(snippets: [a], now: now).rank("meeting").first)
        #expect(match.field == .trigger)
        #expect(match.nameHighlights.isEmpty)
    }

    // MARK: - Performance

    @Test func ranksFiveThousandSnippetsQuickly() {
        let words = ["email", "reply", "address", "signature", "meeting", "invoice", "thanks", "report",
                     "weekly", "status", "follow", "up", "intro", "calendar", "link", "zoom", "phone", "code"]
        var rng = SystemRandomNumberGenerator()
        func phrase(_ n: Int) -> String { (0..<n).map { _ in words.randomElement(using: &rng)! }.joined(separator: " ") }
        let snippets = (0..<5_000).map { i in
            s(phrase(3), [";\(words[i % words.count])\(i)"], phrase(40))
        }
        let clock = ContinuousClock()
        var ranker = PaletteRanker()
        let build = clock.measure { ranker = PaletteRanker(snippets: snippets, now: now) }

        // Simulate typing a query one keystroke at a time, a few times, and keep the best run.
        let query = "meet rep"
        var best = Duration.seconds(100)
        for _ in 0..<3 {
            let elapsed = clock.measure {
                for end in query.indices.dropFirst() {
                    _ = ranker.rank(String(query[..<end]), limit: 200)
                }
                _ = ranker.rank(query, limit: 200)
            }
            best = min(best, elapsed / query.count)
        }
        // Release builds land around 3 ms per keystroke and debug around 50 ms. The limits leave
        // room for loaded CI machines; the real numbers are printed.
        print("PaletteRanker: \(best) per keystroke, \(build) to index 5,000 snippets")
        #if DEBUG
        #expect(best < .milliseconds(500))
        #else
        #expect(best < .milliseconds(50))
        #endif
    }
}
