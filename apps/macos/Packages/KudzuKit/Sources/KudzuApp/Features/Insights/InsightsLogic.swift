import Foundation
import KudzuCore

/// Pure calculations and rankings for `InsightsScreen` (`insights.tsx`).
enum InsightsLogic {
    struct Ranked: Equatable, Sendable, Identifiable {
        var snippet: Snippet
        var usage: SnippetUsage
        var id: UUID { snippet.id }
    }

    struct Report: Equatable, Sendable {
        var totals: Stats.Totals
        var top: [Ranked]
        var maxCount: Int
        var recent: [Ranked]
        var unused: [Snippet]
        var turnedOff: Int
    }

    static let topLimit = 10
    static let recentLimit = 6
    static let unusedLimit = 18

    static func buildReport(library: Library, usage: [UUID: SnippetUsage], wpm: UInt32) -> Report {
        let totals = Stats.totals(library, usage: usage, wpm: Double(wpm))
        let used = library.snippets.compactMap { snippet -> Ranked? in
            guard let record = usage[snippet.id], record.count > 0 else { return nil }
            return Ranked(snippet: snippet, usage: record)
        }
        // The web sorts with a stable `Array.sort` on one key, so ties keep their earlier order:
        // library order for the ranking, ranking order for recent.
        let ranked = stableSorted(used) { $0.usage.count > $1.usage.count }

        let top = Array(ranked.prefix(topLimit))
        let recent = stableSorted(ranked) { $0.usage.last > $1.usage.last }.prefix(recentLimit)
        let unused = library.snippets.filter { snippet in
            snippet.isEnabled && !snippet.triggers.isEmpty && (usage[snippet.id]?.count ?? 0) == 0
        }

        return Report(
            totals: totals,
            top: top,
            maxCount: max(1, top.first?.usage.count ?? 1),
            recent: Array(recent),
            unused: unused,
            turnedOff: library.snippets.count - totals.activeSnippets
        )
    }

    /// A sort that keeps equal elements in their original order, like `Array.prototype.sort`.
    private static func stableSorted<T>(_ items: [T], by areInIncreasingOrder: (T, T) -> Bool) -> [T] {
        items.enumerated()
            .sorted { a, b in
                if areInIncreasingOrder(a.element, b.element) { return true }
                if areInIncreasingOrder(b.element, a.element) { return false }
                return a.offset < b.offset
            }
            .map(\.element)
    }

    /// Clamps typing speed to `10...150` in steps of 5.
    static func clampWPM(_ raw: Int) -> UInt32 {
        let bounded = min(150, max(10, raw))
        let stepped = Int((Double(bounded) / 5.0).rounded()) * 5
        return UInt32(min(150, max(10, stepped)))
    }

    /// Bar width fraction `0...1`. `InsightBar` enforces the 3% floor.
    static func barFraction(count: Int, maxCount: Int) -> Double {
        Double(count) / Double(max(1, maxCount))
    }

    /// `Math.round(chars / CHARS_PER_WORD)`.
    static func wordEstimate(chars: Int) -> Int {
        Int((Double(chars) / Double(Stats.CHARS_PER_WORD)).rounded())
    }

    static func primaryTrigger(for snippet: Snippet) -> String? {
        snippet.triggers.first
    }
}
