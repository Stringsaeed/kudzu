import Foundation

/// The snippet field a palette result matched in.
public enum PaletteField: Sendable, Hashable {
    case name
    case trigger
    case content
}

/// One ranked search-palette result.
public struct PaletteMatch: Sendable, Hashable, Identifiable {
    /// The matched snippet.
    public var snippetID: UUID
    /// Higher is better. Only meaningful relative to other results of the same query.
    public var score: Double
    /// The field that contributed most to the match; nil for an empty query.
    public var field: PaletteField?
    /// Sorted `Character` offsets into the snippet's name that matched, for highlighting.
    public var nameHighlights: [Int]

    public var id: UUID { snippetID }

    /// Creates a result.
    public init(snippetID: UUID, score: Double, field: PaletteField?, nameHighlights: [Int]) {
        self.snippetID = snippetID
        self.score = score
        self.field = field
        self.nameHighlights = nameHighlights
    }
}

/// Fuzzy, usage-boosted ranking for the search palette.
///
/// Build once per library or stats change (it pre-folds every snippet), then call `rank` per
/// keystroke. Each whitespace-separated query term must match a snippet's name, a trigger or its
/// content as a subsequence. Scoring follows the fzf model: every matched character scores, with
/// bonuses for word boundaries, camel humps and runs of consecutive characters, and penalties for
/// gaps. Fields are weighted name > trigger > content, and a query that equals or starts a trigger
/// outranks everything else. Usage (`StatsCalculator.usageRank`) is added on top, scaled so it
/// reorders comparable matches without burying a clearly better one.
public struct PaletteRanker: Sendable {
    // MARK: Tuning

    static let scoreMatch: Int32 = 16
    static let gapStart: Int32 = 3
    static let gapExtension: Int32 = 1
    static let bonusStart: UInt8 = 10
    static let bonusWhitespace: UInt8 = 10
    static let bonusDelimiter: UInt8 = 9
    static let bonusCamel: UInt8 = 7
    static let bonusConsecutive: UInt8 = 4
    /// Bonuses at or above this restart a run's inherited bonus.
    static let bonusBoundaryThreshold: UInt8 = 8
    static let firstCharMultiplier: Int32 = 2
    /// The term matches the field's first characters.
    static let prefixBonus: Int32 = 20
    /// The term is the whole field.
    static let exactBonus: Int32 = 20

    static let nameWeight = 1.0
    static let triggerWeight = 0.85
    static let contentWeight = 0.6
    static let exactTriggerBonus = 10_000.0
    static let prefixTriggerBonus = 5_000.0
    /// Points per unit of `StatsCalculator.usageRank` (roughly 0…10 in practice).
    static let usageWeight = 4.0

    /// Fields longer than this (in UTF-16 units) use the linear greedy matcher instead of the DP.
    static let maxDPColumns = 256
    /// Content beyond this many characters is not searched; it keeps memory and per-keystroke cost bounded.
    static let maxContentCharacters = 2_000
    /// Longer query terms are truncated.
    static let maxTermLength = 64

    // MARK: State

    private let entries: [Entry]

    /// An empty ranker.
    public init() { entries = [] }

    /// Number of searchable (enabled) snippets.
    public var count: Int { entries.count }

    /// Indexes the enabled snippets. A snippet is excluded when it, or any group above it, is disabled.
    public init(snippets: [SnippetRecord], groups: [GroupRecord] = [], stats: [UUID: SnippetStats] = [:], now: Date = .now) {
        let tree = GroupTree(groups: groups)
        var entries: [Entry] = []
        entries.reserveCapacity(snippets.count)
        for snippet in snippets where snippet.isEnabled {
            if let groupID = snippet.groupID, tree.group(groupID) != nil, !tree.isEffectivelyEnabled(groupID) { continue }
            let usage = stats[snippet.id].map {
                StatsCalculator.usageRank(count: $0.count, lastUsedAt: $0.lastUsedAt, now: now)
            } ?? 0
            entries.append(Entry(snippet, usage: usage))
        }
        // Pre-sorted in empty-query order, so browsing needs no work per keystroke.
        entries.sort { a, b in
            if a.isFavorite != b.isFavorite { return a.isFavorite }
            return Self.tieBreak(a, b)
        }
        self.entries = entries
    }

    /// Ranked matches for `query`, best first, at most `limit` of them.
    ///
    /// An empty query lists every snippet: favorites first, then by usage, then by name.
    public func rank(_ query: String, limit: Int = .max) -> [PaletteMatch] {
        let limit = max(0, limit)
        let whole = SnippetSearchIndex.fold(query.trimmingCharacters(in: .whitespacesAndNewlines))
        guard !whole.isEmpty else {
            return entries.prefix(limit).map { PaletteMatch(snippetID: $0.id, score: $0.usage * Self.usageWeight, field: nil, nameHighlights: []) }
        }
        let terms = SnippetSearchIndex.terms(from: query).map { Array($0.utf16.prefix(Self.maxTermLength)) }
        var scratch = Scratch(rows: terms.map(\.count).max() ?? 1)
        var candidates: [Candidate] = []

        for index in entries.indices {
            let entry = entries[index]
            var triggerBonus = 0.0
            if entry.triggerKeys.contains(whole) {
                triggerBonus = Self.exactTriggerBonus
            } else if entry.triggerKeys.contains(where: { $0.hasPrefix(whole) }) {
                triggerBonus = Self.prefixTriggerBonus
            }

            var total = 0.0
            var byField: [Double] = [0, 0, 0]
            var highlights: [Int] = []
            var matchedAll = true
            for term in terms {
                guard let hit = Self.match(term, in: entry, scratch: &scratch) else {
                    matchedAll = false
                    break
                }
                total += hit.score
                byField[hit.field.slot] += hit.score
                if hit.field == .name { highlights += scratch.namePositions.map { Int(entry.name.owners[$0]) } }
            }
            guard matchedAll || triggerBonus > 0 else { continue }

            let field: PaletteField
            if triggerBonus > 0 {
                field = .trigger
            } else if byField[0] >= byField[1], byField[0] >= byField[2] {
                field = .name
            } else {
                field = byField[1] >= byField[2] ? .trigger : .content
            }
            if highlights.count > 1 { highlights = Array(Set(highlights)).sorted() }
            candidates.append(Candidate(
                index: index,
                score: total + triggerBonus + entry.usage * Self.usageWeight,
                field: field,
                highlights: highlights
            ))
        }

        candidates.sort { a, b in
            if a.score != b.score { return a.score > b.score }
            return Self.tieBreak(entries[a.index], entries[b.index])
        }
        return candidates.prefix(limit).map {
            PaletteMatch(snippetID: entries[$0.index].id, score: $0.score, field: $0.field, nameHighlights: $0.highlights)
        }
    }

    private static func tieBreak(_ a: Entry, _ b: Entry) -> Bool {
        if a.usage != b.usage { return a.usage > b.usage }
        if a.sortKey != b.sortKey { return a.sortKey < b.sortKey }
        return a.idString < b.idString
    }

    // MARK: - Per-term matching

    private struct Hit {
        var score: Double
        var field: PaletteField
    }

    /// Best weighted match of one term across name, triggers and content.
    /// A gappy name hit can score below a tight content hit, so content is always considered;
    /// its span limit keeps that scan cheap.
    /// On a name hit, `scratch.namePositions` holds the matched UTF-16 offsets.
    private static func match(_ term: [UInt16], in entry: Entry, scratch: inout Scratch) -> Hit? {
        var best: Hit?
        if let score = fuzzy(term, entry.name, scratch: &scratch, keepPositions: true) {
            best = Hit(score: Double(score) * nameWeight, field: .name)
        }
        for trigger in entry.triggers {
            guard let score = fuzzy(term, trigger, scratch: &scratch, keepPositions: false) else { continue }
            let weighted = Double(score) * triggerWeight
            if weighted > best?.score ?? -.infinity { best = Hit(score: weighted, field: .trigger) }
        }
        let maxSpan = term.count + max(3, term.count)
        if let score = greedy(term, entry.content, bonus: nil, maxSpan: maxSpan) {
            let weighted = Double(score) * contentWeight
            if weighted > best?.score ?? -.infinity { best = Hit(score: weighted, field: .content) }
        }
        return best
    }

    /// Scores a term against a short field with the optimal-alignment DP, or the greedy matcher
    /// for long fields.
    private static func fuzzy(_ term: [UInt16], _ field: FoldedField, scratch: inout Scratch, keepPositions: Bool) -> Int32? {
        let m = term.count, n = field.units.count
        guard m > 0, m <= n else { return nil }
        guard n <= maxDPColumns else {
            if keepPositions { scratch.namePositions = [] }
            return greedy(term, field.units, bonus: field.bonus, maxSpan: nil)
        }

        return term.withUnsafeBufferPointer { q in
            field.units.withUnsafeBufferPointer { t in
                field.bonus.withUnsafeBufferPointer { bonus in
                    guard isSubsequence(q, of: t) else { return nil }
                    let score = scratch.align(q, t, bonus)
                    if keepPositions { scratch.namePositions = scratch.positions }
                    return score
                }
            }
        }
    }

    private static func isSubsequence(_ q: UnsafeBufferPointer<UInt16>, of t: UnsafeBufferPointer<UInt16>) -> Bool {
        var j = 0
        for unit in t where unit == q[j] {
            j += 1
            if j == q.count { return true }
        }
        return false
    }

    /// Linear-time matching for long text: the best-placed exact occurrence if there is one,
    /// otherwise the first subsequence occurrence, tightened from the right (fzf's v1 algorithm).
    /// `maxSpan` rejects matches whose letters are scattered too far apart to be meaningful.
    private static func greedy(_ term: [UInt16], _ text: [UInt16], bonus: [UInt8]?, maxSpan: Int?) -> Int32? {
        let m = term.count, n = text.count
        guard m > 0, m <= n else { return nil }
        return term.withUnsafeBufferPointer { q in
            text.withUnsafeBufferPointer { t in
                let bonusAt: (Int) -> UInt8 = { i in
                    if let bonus { return bonus[i] }
                    return boundaryBonus(prev: i == 0 ? .start : CharClass(folded: t[i - 1]), current: CharClass(folded: t[i]))
                }

                // Exact occurrence, preferring one that starts on a word boundary.
                var bestStart = -1
                var bestStartBonus: UInt8 = 0
                var i = 0
                while i <= n - m {
                    if t[i] == q[0] {
                        var k = 1
                        while k < m, t[i + k] == q[k] { k += 1 }
                        if k == m {
                            let b = bonusAt(i)
                            if bestStart < 0 || b > bestStartBonus { bestStart = i; bestStartBonus = b }
                            if b >= bonusBoundaryThreshold { break }
                        }
                    }
                    i += 1
                }
                if bestStart >= 0 {
                    var score = scoreRun(start: bestStart, count: m, bonusAt: bonusAt)
                    if bestStart == 0 { score += prefixBonus + (n == m ? exactBonus : 0) }
                    return score
                }

                // Subsequence: earliest end, then the latest start that still reaches it.
                var j = 0, end = -1
                for i in 0..<n where t[i] == q[j] {
                    j += 1
                    if j == m { end = i; break }
                }
                guard end >= 0 else { return nil }
                j = m - 1
                var start = end
                for i in stride(from: end, through: 0, by: -1) where t[i] == q[j] {
                    if j == 0 { start = i; break }
                    j -= 1
                }
                if let maxSpan, end - start + 1 > maxSpan { return nil }

                var score: Int32 = 0
                var chunk: UInt8 = 0
                var previous = -2
                j = 0
                for i in start...end where j < m && t[i] == q[j] {
                    score += step(index: i, previous: previous, first: j == 0, bonus: bonusAt(i), chunk: &chunk)
                    previous = i
                    j += 1
                }
                return score
            }
        }
    }

    private static func scoreRun(start: Int, count: Int, bonusAt: (Int) -> UInt8) -> Int32 {
        var score: Int32 = 0
        var chunk: UInt8 = 0
        for offset in 0..<count {
            score += step(index: start + offset, previous: start + offset - 1, first: offset == 0, bonus: bonusAt(start + offset), chunk: &chunk)
        }
        return score
    }

    /// Score for matching one character at `index`, given the previous match position.
    private static func step(index: Int, previous: Int, first: Bool, bonus: UInt8, chunk: inout UInt8) -> Int32 {
        if first {
            chunk = bonus
            return scoreMatch + Int32(bonus) * firstCharMultiplier
        }
        if index == previous + 1 {
            if bonus >= bonusBoundaryThreshold { chunk = bonus }
            return scoreMatch + Int32(max(chunk, bonusConsecutive, bonus))
        }
        chunk = bonus
        let gap = Int32(index - previous - 1)
        return scoreMatch + Int32(bonus) - gapStart - gapExtension * (gap - 1)
    }

    // MARK: - Character classes

    enum CharClass: UInt8 {
        case start, whitespace, delimiter, lower, upper, number, letter

        init(_ c: Character) {
            if let ascii = c.asciiValue {
                self = ascii >= 65 && ascii <= 90 ? .upper : CharClass(folded: UInt16(ascii))
            } else if c.isWhitespace {
                self = .whitespace
            } else if c.isNumber {
                self = .number
            } else if c.isUppercase {
                self = .upper
            } else if c.isLowercase {
                self = .lower
            } else if c.isLetter {
                self = .letter
            } else {
                self = .delimiter
            }
        }

        /// Classifies an already-folded UTF-16 unit, so there is no uppercase.
        init(folded unit: UInt16) {
            switch unit {
            case 9, 10, 11, 12, 13, 32: self = .whitespace
            case 48...57: self = .number
            case 65...90, 97...122: self = .lower
            case 0..<128: self = .delimiter
            default: self = .letter
            }
        }
    }


    static func boundaryBonus(prev: CharClass, current: CharClass) -> UInt8 {
        if prev == .start { return bonusStart }
        switch current {
        case .whitespace, .delimiter, .start:
            return 0
        case .lower, .upper, .number, .letter:
            if prev == .whitespace { return bonusWhitespace }
            if prev == .delimiter { return bonusDelimiter }
            if prev == .lower, current == .upper { return bonusCamel }
            if prev != .number, current == .number { return bonusCamel }
            return 0
        }
    }

    // MARK: - Index storage

    /// Folded UTF-16 text with a per-unit boundary bonus and, for names, the owning `Character` offset.
    struct FoldedField: Sendable {
        var units: [UInt16] = []
        var bonus: [UInt8] = []
        var owners: [Int32] = []

        init(_ text: String, trackOwners: Bool) {
            units.reserveCapacity(text.utf16.count)
            bonus.reserveCapacity(text.utf16.count)
            if trackOwners { owners.reserveCapacity(text.utf16.count) }
            var previous = CharClass.start
            var offset: Int32 = 0
            for character in text {
                let current = CharClass(character)
                let b = boundaryBonus(prev: previous, current: current)
                if let ascii = character.asciiValue {
                    units.append(UInt16(ascii >= 65 && ascii <= 90 ? ascii + 32 : ascii))
                    bonus.append(b)
                    if trackOwners { owners.append(offset) }
                } else {
                    var first = true
                    for unit in SnippetSearchIndex.fold(String(character)).utf16 {
                        units.append(unit)
                        bonus.append(first ? b : 0)
                        if trackOwners { owners.append(offset) }
                        first = false
                    }
                }
                previous = current
                offset += 1
            }
        }
    }

    private struct Entry: Sendable {
        var id: UUID
        var idString: String
        var name: FoldedField
        var triggers: [FoldedField]
        /// Folded triggers, plus each without its leading punctuation (`;sig` → `sig`).
        var triggerKeys: [String]
        var content: [UInt16]
        var sortKey: String
        var usage: Double
        var isFavorite: Bool

        init(_ snippet: SnippetRecord, usage: Double) {
            id = snippet.id
            idString = snippet.id.uuidString
            name = FoldedField(snippet.name, trackOwners: true)
            triggers = snippet.triggers.map { FoldedField($0, trackOwners: false) }
            var keys: [String] = []
            for trigger in snippet.triggers {
                let folded = SnippetSearchIndex.fold(trigger)
                keys.append(folded)
                let core = String(folded.drop { !$0.isLetter && !$0.isNumber })
                if !core.isEmpty, core != folded { keys.append(core) }
            }
            triggerKeys = keys
            content = Array(SnippetSearchIndex.fold(String(snippet.content.prefix(PaletteRanker.maxContentCharacters))).utf16)
            sortKey = SnippetSearchIndex.fold(snippet.name)
            self.usage = usage
            isFavorite = snippet.isFavorite
        }
    }

    private struct Candidate {
        var index: Int
        var score: Double
        var field: PaletteField
        var highlights: [Int]
    }

    // MARK: - Alignment DP

    /// Reusable buffers for the per-field DP, so ranking thousands of snippets doesn't allocate per field.
    private struct Scratch {
        var score: [Int32]
        var chunk: [UInt8]
        var from: [Int16]
        /// Matched offsets from the last `align`.
        var positions: [Int] = []
        /// Matched offsets of the last name hit.
        var namePositions: [Int] = []

        init(rows: Int) {
            let size = max(1, rows) * PaletteRanker.maxDPColumns
            score = Array(repeating: 0, count: size)
            chunk = Array(repeating: 0, count: size)
            from = Array(repeating: -1, count: size)
        }

        /// Best-scoring alignment of `q` in `t` (which must contain it as a subsequence).
        ///
        /// `score[j][i]` is the best score with `q[j]` matched at `t[i]`. A match either extends a
        /// run (`q[j-1]` at `i-1`, earning the run's inherited bonus) or follows a gap, whose
        /// best predecessor is tracked incrementally with the affine gap penalty.
        mutating func align(_ q: UnsafeBufferPointer<UInt16>, _ t: UnsafeBufferPointer<UInt16>, _ bonus: UnsafeBufferPointer<UInt8>) -> Int32 {
            let m = q.count, n = t.count
            let none = Int32.min / 2
            let result: (Int32, Int) = score.withUnsafeMutableBufferPointer { S in
                chunk.withUnsafeMutableBufferPointer { C in
                    from.withUnsafeMutableBufferPointer { F in
                        for i in 0..<n {
                            if t[i] == q[0] {
                                S[i] = PaletteRanker.scoreMatch + Int32(bonus[i]) * PaletteRanker.firstCharMultiplier
                                C[i] = bonus[i]
                            } else {
                                S[i] = none
                            }
                            F[i] = -1
                        }
                        if m > 1 {
                            for j in 1..<m {
                                let row = j * n, prevRow = (j - 1) * n
                                var gapBest = none
                                var gapFrom = -1
                                for i in 0..<n {
                                    if i >= 2 {
                                        if gapBest > none { gapBest -= PaletteRanker.gapExtension }
                                        let p = S[prevRow + i - 2]
                                        if p > none, p - PaletteRanker.gapStart >= gapBest {
                                            gapBest = p - PaletteRanker.gapStart
                                            gapFrom = i - 2
                                        }
                                    }
                                    guard i >= j, t[i] == q[j] else { S[row + i] = none; continue }
                                    var best = none
                                    var origin = -1
                                    var run = bonus[i]
                                    let p = S[prevRow + i - 1]
                                    if p > none {
                                        let inherited = bonus[i] >= PaletteRanker.bonusBoundaryThreshold ? bonus[i] : C[prevRow + i - 1]
                                        best = p + PaletteRanker.scoreMatch + Int32(max(inherited, PaletteRanker.bonusConsecutive, bonus[i]))
                                        origin = i - 1
                                        run = inherited
                                    }
                                    if gapBest > none {
                                        let candidate = gapBest + PaletteRanker.scoreMatch + Int32(bonus[i])
                                        if candidate > best {
                                            best = candidate
                                            origin = gapFrom
                                            run = bonus[i]
                                        }
                                    }
                                    S[row + i] = best
                                    C[row + i] = run
                                    F[row + i] = Int16(origin)
                                }
                            }
                        }
                        let last = (m - 1) * n
                        var bestScore = none, bestEnd = -1
                        for i in (m - 1)..<n where S[last + i] > bestScore {
                            bestScore = S[last + i]
                            bestEnd = i
                        }
                        return (bestScore, bestEnd)
                    }
                }
            }

            positions.removeAll(keepingCapacity: true)
            positions.append(contentsOf: repeatElement(0, count: m))
            positions[m - 1] = result.1
            if m > 1 {
                for j in stride(from: m - 1, to: 0, by: -1) {
                    positions[j - 1] = Int(from[j * n + positions[j]])
                }
            }
            var total = result.0
            if positions[0] == 0, positions[m - 1] == m - 1 {
                total += PaletteRanker.prefixBonus + (n == m ? PaletteRanker.exactBonus : 0)
            }
            return total
        }
    }
}

private extension PaletteField {
    var slot: Int {
        switch self {
        case .name: 0
        case .trigger: 1
        case .content: 2
        }
    }
}
