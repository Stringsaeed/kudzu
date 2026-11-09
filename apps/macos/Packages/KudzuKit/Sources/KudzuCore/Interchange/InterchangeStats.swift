// Ported from kudzu-core/src/interchange/stats.rs
// Usage statistics: per-snippet totals across devices, time saved and palette ranking.

import Foundation

/// Usage totals across devices, estimated typing time saved and palette ranking.
public enum InterchangeStats {
    public static let CHARACTERS_PER_WORD = 5.0, RECENCY_HALF_LIFE_DAYS = 7.0, RECENCY_WEIGHT = 2.0
    public struct SnippetStats: Equatable, Sendable { public var snippetID: UUID; public var count: Int; public var charsSaved: Int; public var lastUsedAt: Date? }
    public struct Summary: Codable, Equatable, Sendable {
        public var perSnippet: [SnippetStats]; public var totalExpansions: Int; public var totalCharsSaved: Int; public var estimatedTimeSaved: Double
        /// The `n` most-used snippets, excluding ones never used.
        public func topSnippets(_ n: Int) -> [SnippetStats] { Array(perSnippet.filter { $0.count > 0 }.prefix(max(0,n))) }
        /// Palette ranking score for a snippet; 0 when it is unknown or has never been used.
        public func usageRankFor(_ id: UUID, now: Date) -> Double { perSnippet.first { $0.snippetID == id }.map { InterchangeStats.usageRank(count:$0.count,lastUsedAt:$0.lastUsedAt,now:now) } ?? 0 }
        /// Palette ranking scores for every snippet, for ranking many results at once.
        public func usageRanks(now: Date) -> [UUID:Double] { Dictionary(uniqueKeysWithValues:perSnippet.map { ($0.snippetID,InterchangeStats.usageRank(count:$0.count,lastUsedAt:$0.lastUsedAt,now:now)) }) }
    }
    /// Sums per-device usage into per-snippet stats. Usage of snippets not in `snippets` is ignored.
    public static func summary(usage: [UsageRecord], snippets: [Snippet], typingSpeedWPM: UInt32) -> Summary {
        var rows: [UUID:SnippetStats] = [:]
        for s in snippets { rows[s.id] = SnippetStats(snippetID:s.id,count:0,charsSaved:0,lastUsedAt:nil) }
        for u in usage where rows[u.snippetID] != nil {
            rows[u.snippetID]?.count += u.count; rows[u.snippetID]?.charsSaved += u.charsSaved
            let latest = [rows[u.snippetID]?.lastUsedAt,u.lastUsedAt].compactMap { $0 }.max()
            rows[u.snippetID]?.lastUsedAt = latest
        }
        let sorted = rows.values.sorted {
            if $0.count != $1.count { return $0.count > $1.count }; if $0.charsSaved != $1.charsSaved { return $0.charsSaved > $1.charsSaved }
            if $0.lastUsedAt != $1.lastUsedAt { return ($0.lastUsedAt ?? .distantPast) > ($1.lastUsedAt ?? .distantPast) }; return $0.snippetID.lowercased < $1.snippetID.lowercased
        }
        let expansions = sorted.reduce(Int(0)) { $0+$1.count }, chars = sorted.reduce(Int(0)) { $0+$1.charsSaved }
        return Summary(perSnippet:sorted,totalExpansions:expansions,totalCharsSaved:chars,estimatedTimeSaved:timeSaved(chars:chars,typingSpeedWPM:typingSpeedWPM))
    }
    /// Seconds it would take to type `chars` characters at `typing_speed_wpm` (chars / 5 = words).
    public static func timeSaved(chars: Int, typingSpeedWPM: UInt32) -> Double { typingSpeedWPM == 0 || chars <= 0 ? 0 : Double(chars)/5/Double(typingSpeedWPM)*60 }
    /// Search-palette score: `ln(1 + count) + RECENCY_WEIGHT × 0.5^(ageDays / RECENCY_HALF_LIFE_DAYS)`.
    ///
    /// The log keeps heavy hitters from drowning everything else; the boost (2.0 just now, 1.0 after a
    /// week, ~0 after a couple of months) lets something used today outrank something used somewhat
    /// more often long ago. Future `last_used_at` (clock skew across devices) counts as "now".
    public static func usageRank(count: Int, lastUsedAt: Date?, now: Date) -> Double {
        let frequency = log1p(Double(max(0,count)))
        guard let lastUsedAt else { return frequency }; let age = max(0,(now.timeIntervalSince(lastUsedAt)*1000).rounded(.towardZero))/1000/86400
        return frequency+2*pow(0.5,age/7)
    }
}
