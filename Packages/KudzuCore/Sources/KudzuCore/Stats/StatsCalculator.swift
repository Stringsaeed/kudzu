import Foundation

/// Usage totals for one snippet, summed across every device.
public struct SnippetStats: Hashable, Sendable, Identifiable {
    public var snippetID: UUID
    public var count: Int
    public var charsSaved: Int
    public var lastUsedAt: Date?

    public var id: UUID { snippetID }

    public init(snippetID: UUID, count: Int = 0, charsSaved: Int = 0, lastUsedAt: Date? = nil) {
        self.snippetID = snippetID
        self.count = count
        self.charsSaved = charsSaved
        self.lastUsedAt = lastUsedAt
    }
}

/// Library-wide usage statistics, as shown in the Stats window.
public struct StatsSummary: Hashable, Sendable {
    /// Every known snippet, sorted by count, then chars saved, then recency (descending), then id.
    public var perSnippet: [SnippetStats]
    public var totalExpansions: Int
    public var totalCharsSaved: Int
    /// Seconds of typing saved at the configured typing speed.
    public var estimatedTimeSaved: TimeInterval

    public init(perSnippet: [SnippetStats], totalExpansions: Int, totalCharsSaved: Int, estimatedTimeSaved: TimeInterval) {
        self.perSnippet = perSnippet
        self.totalExpansions = totalExpansions
        self.totalCharsSaved = totalCharsSaved
        self.estimatedTimeSaved = estimatedTimeSaved
    }

    /// The `n` most-used snippets, excluding ones never used.
    public func topSnippets(_ n: Int) -> [SnippetStats] {
        Array(perSnippet.lazy.filter { $0.count > 0 }.prefix(max(0, n)))
    }

    /// Palette ranking score for a snippet; 0 when it has never been used. See `StatsCalculator.usageRank`.
    public func usageRank(for snippetID: UUID, now: Date = .now) -> Double {
        guard let stats = perSnippet.first(where: { $0.snippetID == snippetID }) else { return 0 }
        return StatsCalculator.usageRank(count: stats.count, lastUsedAt: stats.lastUsedAt, now: now)
    }

    /// Palette ranking scores for every snippet, for ranking many results at once.
    public func usageRanks(now: Date = .now) -> [UUID: Double] {
        Dictionary(uniqueKeysWithValues: perSnippet.map {
            ($0.snippetID, StatsCalculator.usageRank(count: $0.count, lastUsedAt: $0.lastUsedAt, now: now))
        })
    }
}

/// Pure statistics over usage records; no persistence.
public enum StatsCalculator {
    /// Average characters per word used for the time-saved estimate (the conventional WPM definition).
    public static let charactersPerWord = 5.0
    /// Days after which the recency boost in `usageRank` halves.
    public static let recencyHalfLifeDays = 7.0
    /// Weight of the recency boost relative to `ln(1 + count)`.
    public static let recencyWeight = 2.0

    /// Sums per-device usage into per-snippet stats. Usage of snippets not in `snippets` is ignored.
    public static func summary(usage: [UsageRecord], snippets: [SnippetRecord], typingSpeedWPM: Int) -> StatsSummary {
        var bySnippet: [UUID: SnippetStats] = [:]
        for snippet in snippets {
            bySnippet[snippet.id] = SnippetStats(snippetID: snippet.id)
        }
        for row in usage {
            guard var stats = bySnippet[row.snippetID] else { continue }
            stats.count += row.count
            stats.charsSaved += row.charsSaved
            stats.lastUsedAt = latest(stats.lastUsedAt, row.lastUsedAt)
            bySnippet[row.snippetID] = stats
        }

        let perSnippet = bySnippet.values.sorted { lhs, rhs in
            if lhs.count != rhs.count { return lhs.count > rhs.count }
            if lhs.charsSaved != rhs.charsSaved { return lhs.charsSaved > rhs.charsSaved }
            let l = lhs.lastUsedAt ?? .distantPast, r = rhs.lastUsedAt ?? .distantPast
            if l != r { return l > r }
            return lhs.snippetID.uuidString < rhs.snippetID.uuidString
        }
        let totalExpansions = perSnippet.reduce(0) { $0 + $1.count }
        let totalCharsSaved = perSnippet.reduce(0) { $0 + $1.charsSaved }
        return StatsSummary(
            perSnippet: perSnippet,
            totalExpansions: totalExpansions,
            totalCharsSaved: totalCharsSaved,
            estimatedTimeSaved: timeSaved(chars: totalCharsSaved, typingSpeedWPM: typingSpeedWPM)
        )
    }

    /// Seconds it would take to type `chars` characters at `typingSpeedWPM` (chars / 5 = words).
    public static func timeSaved(chars: Int, typingSpeedWPM: Int) -> TimeInterval {
        guard typingSpeedWPM > 0, chars > 0 else { return 0 }
        return Double(chars) / charactersPerWord / Double(typingSpeedWPM) * 60
    }

    /// Search-palette ranking score: frequency with a decaying recency boost.
    ///
    /// `rank = ln(1 + count) + recencyWeight × 0.5^(ageDays / recencyHalfLifeDays)`
    ///
    /// The log keeps heavy hitters from drowning everything else; the boost
    /// (2.0 when used just now, 1.0 after a week, ~0 after a couple of months)
    /// lets a snippet used today outrank one used somewhat more often long ago.
    /// Never-used snippets score 0. Future `lastUsedAt` (clock skew across
    /// devices) is treated as "now".
    public static func usageRank(count: Int, lastUsedAt: Date?, now: Date) -> Double {
        let frequency = log(1 + Double(max(0, count)))
        guard let lastUsedAt else { return frequency }
        let ageDays = max(0, now.timeIntervalSince(lastUsedAt)) / 86_400
        return frequency + recencyWeight * pow(0.5, ageDays / recencyHalfLifeDays)
    }

    static func latest(_ a: Date?, _ b: Date?) -> Date? {
        switch (a, b) {
        case let (a?, b?): max(a, b)
        case let (a?, nil): a
        case let (nil, b): b
        }
    }
}
