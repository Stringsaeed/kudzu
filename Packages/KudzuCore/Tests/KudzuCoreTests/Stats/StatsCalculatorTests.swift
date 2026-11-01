import Foundation
import Testing
@testable import KudzuCore

@Suite struct StatsCalculatorTests {
    let a = SnippetRecord(id: UUID(), name: "A", triggers: [";a"], content: "alpha")
    let b = SnippetRecord(id: UUID(), name: "B", triggers: [";b"], content: "beta")
    let c = SnippetRecord(id: UUID(), name: "C", triggers: [";c"], content: "gamma")
    let t0 = Date(timeIntervalSinceReferenceDate: 1_000_000)

    @Test func sumsAcrossDevicesAndSortsByCountDescending() {
        let usage = [
            UsageRecord(snippetID: a.id, deviceID: "mac1", count: 2, charsSaved: 20, lastUsedAt: t0),
            UsageRecord(snippetID: a.id, deviceID: "mac2", count: 3, charsSaved: 30, lastUsedAt: t0.addingTimeInterval(60)),
            UsageRecord(snippetID: b.id, deviceID: "mac1", count: 10, charsSaved: 5, lastUsedAt: t0),
        ]
        let summary = StatsCalculator.summary(usage: usage, snippets: [a, b, c], typingSpeedWPM: 40)

        #expect(summary.perSnippet.map(\.snippetID) == [b.id, a.id, c.id])
        let statsA = summary.perSnippet[1]
        #expect(statsA.count == 5)
        #expect(statsA.charsSaved == 50)
        #expect(statsA.lastUsedAt == t0.addingTimeInterval(60))
        #expect(summary.perSnippet[2].count == 0)
        #expect(summary.perSnippet[2].lastUsedAt == nil)
        #expect(summary.totalExpansions == 15)
        #expect(summary.totalCharsSaved == 55)
    }

    @Test func estimatesTimeSavedFromWPM() {
        // 1000 chars = 200 words; at 40 WPM that is 5 minutes.
        let usage = [UsageRecord(snippetID: a.id, deviceID: "m", count: 1, charsSaved: 1000)]
        let summary = StatsCalculator.summary(usage: usage, snippets: [a], typingSpeedWPM: 40)
        #expect(summary.estimatedTimeSaved == 300)
        #expect(StatsCalculator.summary(usage: usage, snippets: [a], typingSpeedWPM: 0).estimatedTimeSaved == 0)
    }

    @Test func ignoresUsageForUnknownSnippets() {
        let usage = [UsageRecord(snippetID: UUID(), deviceID: "m", count: 7, charsSaved: 70)]
        let summary = StatsCalculator.summary(usage: usage, snippets: [a], typingSpeedWPM: 40)
        #expect(summary.totalExpansions == 0)
        #expect(summary.perSnippet.count == 1)
    }

    @Test func topSnippetsSkipsUnusedAndLimits() {
        let usage = [
            UsageRecord(snippetID: a.id, deviceID: "m", count: 1),
            UsageRecord(snippetID: b.id, deviceID: "m", count: 2),
        ]
        let summary = StatsCalculator.summary(usage: usage, snippets: [a, b, c], typingSpeedWPM: 40)
        #expect(summary.topSnippets(1).map(\.snippetID) == [b.id])
        #expect(summary.topSnippets(10).map(\.snippetID) == [b.id, a.id])
    }

    @Test func tiesAreBrokenDeterministically() {
        let usage = [
            UsageRecord(snippetID: a.id, deviceID: "m", count: 1, charsSaved: 1, lastUsedAt: t0),
            UsageRecord(snippetID: b.id, deviceID: "m", count: 1, charsSaved: 1, lastUsedAt: t0),
        ]
        let first = StatsCalculator.summary(usage: usage, snippets: [a, b], typingSpeedWPM: 40)
        let second = StatsCalculator.summary(usage: usage.reversed(), snippets: [b, a], typingSpeedWPM: 40)
        #expect(first.perSnippet == second.perSnippet)
    }

    @Test func usageRankFormula() {
        #expect(StatsCalculator.usageRank(count: 0, lastUsedAt: nil, now: t0) == 0)
        // Used just now: ln(1 + 1) + 2 * 1.
        let fresh = StatsCalculator.usageRank(count: 1, lastUsedAt: t0, now: t0)
        #expect(abs(fresh - (log(2) + 2)) < 1e-9)
        // One half-life (7 days) later the recency boost halves.
        let weekOld = StatsCalculator.usageRank(count: 1, lastUsedAt: t0, now: t0.addingTimeInterval(7 * 86_400))
        #expect(abs(weekOld - (log(2) + 1)) < 1e-9)
        // Future timestamps (clock skew between devices) don't exceed the "just used" boost.
        let future = StatsCalculator.usageRank(count: 1, lastUsedAt: t0.addingTimeInterval(3600), now: t0)
        #expect(abs(future - fresh) < 1e-9)
    }

    @Test func recentBeatsSlightlyMoreFrequentButStale() {
        let now = t0.addingTimeInterval(60 * 86_400)
        let usage = [
            UsageRecord(snippetID: a.id, deviceID: "m", count: 12, lastUsedAt: t0),
            UsageRecord(snippetID: b.id, deviceID: "m", count: 8, lastUsedAt: now),
        ]
        let summary = StatsCalculator.summary(usage: usage, snippets: [a, b], typingSpeedWPM: 40)
        #expect(summary.usageRank(for: b.id, now: now) > summary.usageRank(for: a.id, now: now))
        #expect(summary.usageRank(for: c.id, now: now) == 0)
    }
}
