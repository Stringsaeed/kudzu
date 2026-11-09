import Foundation

public struct SearchEntry: Sendable {
    public var snippet: Snippet
    public var name: String
    public var nameLower: String
    public var triggersLower: [String]
    public var contentLower: String
    public var usage: Int
    /// Milliseconds since the Unix epoch, matching stats.ts.
    public var lastUsed: Double
}
public struct SearchHit: Sendable {
    public enum Field: String, Sendable { case name, trigger, content }
    public var snippet: Snippet
    public var score: Double
    public var field: Field?
    /// UTF-16 indices into the name that matched.
    public var nameHits: [Int]
}

/// Fuzzy, usage-boosted snippet search shared by the palette and library list.
/// The index pre-folds every field once, so a keystroke is a tight loop over strings.
public struct SearchIndex: Sendable {
    public private(set) var entries: [SearchEntry] = []
    private struct Folded: Sendable {
        var stamp: Date; var name: String; var triggers: [String]; var contentLength: Int
        var displayName: String; var lowerName: String; var lowerTriggers: [String]; var lowerContent: String
    }
    private var folded: [UUID: Folded] = [:]
    public init(snippets: [Snippet], usage: [UUID: SnippetUsage] = [:]) { update(snippets: snippets, usage: usage) }
    /// Usage-only refreshes reuse the folded fields, as in search.ts, without shared mutable global state.
    public mutating func update(snippets: [Snippet], usage: [UUID: SnippetUsage] = [:]) {
        let ids = Set(snippets.map(\.id)); folded = folded.filter { ids.contains($0.key) }
        entries = snippets.map { s in
            let display = Self.displayName(s)
            var f = folded[s.id]
            if f == nil || f?.stamp != s.updatedAt || !(f?.name.utf16.elementsEqual(s.name.utf16) ?? false) || !(f?.triggers.count == s.triggers.count && zip(f?.triggers ?? [],s.triggers).allSatisfy { $0.utf16.elementsEqual($1.utf16) }) || f?.contentLength != s.content.utf16.count {
                f = Folded(stamp:s.updatedAt,name:s.name,triggers:s.triggers,contentLength:s.content.utf16.count,displayName:display,lowerName:display.lowercased(),lowerTriggers:s.triggers.map { $0.lowercased() },lowerContent:Template.plainSummary(s.content,max:2000).lowercased())
                folded[s.id] = f
            }
            return SearchEntry(snippet:s,name:f?.displayName ?? display,nameLower:f?.lowerName ?? "",triggersLower:f?.lowerTriggers ?? [],contentLower:f?.lowerContent ?? "",usage:usage[s.id]?.count ?? 0,lastUsed:usage[s.id]?.last ?? 0)
        }
    }
    public static func buildIndex(_ snippets: [Snippet], usage: [UUID: SnippetUsage] = [:]) -> [SearchEntry] {
        snippets.map { s in
            let name = displayName(s)
            return SearchEntry(snippet: s, name: name, nameLower: name.lowercased(), triggersLower: s.triggers.map { $0.lowercased() }, contentLower: Template.plainSummary(s.content, max: 2000).lowercased(), usage: usage[s.id]?.count ?? 0, lastUsed: usage[s.id]?.last ?? 0)
        }
    }
    public func search(_ query: String, limit: Int = 200, now: Date = Date()) -> [SearchHit] { Self.search(entries, query: query, limit: limit, now: now) }
    /// `s.name || s.triggers[0] || "Untitled"`: an empty first trigger falls through too.
    static func displayName(_ s: Snippet) -> String {
        if !s.name.isEmpty { return s.name }
        if let first = s.triggers.first, !first.isEmpty { return first }
        return "Untitled"
    }
    /// Each term must match the name, trigger or contiguous content. Exact triggers always win.
    public static func search(_ entries: [SearchEntry], query: String, limit: Int = 200, now: Date = Date()) -> [SearchHit] {
        // JavaScript `trim()` and `split(/\s+/)`.
        let q = ECMAScriptWhitespace.trim(query).lowercased()
        let terms = ECMAScriptWhitespace.split(q).map { String(decoding: $0.utf16.prefix(64), as: UTF16.self) }
        var hits: [SearchHit] = []
        for e in entries {
            var total = 0.0, best = -Double.infinity, field: SearchHit.Field?, positions: [Int] = [], ok = true
            for term in terms {
                var termBest = -Double.infinity, termField: SearchHit.Field?
                if let n = fuzzy(term, e.nameLower, wantPositions: true) { termBest = n.score; termField = .name; positions += n.positions }
                for trigger in e.triggersLower {
                    if trigger.utf16.elementsEqual(term.utf16) { termBest = max(termBest, 10_000); termField = .trigger; continue }
                    if trigger.utf16.prefix(term.utf16.count).elementsEqual(term.utf16) {
                        if 5000 > termBest { termBest = 5000 - Double(trigger.utf16.count); termField = .trigger }; continue
                    }
                    if let r = fuzzy(term, trigger, wantPositions: false), r.score * 0.85 > termBest { termBest = r.score * 0.85; termField = .trigger }
                }
                if termBest < 40, term.utf16.count > 1, let at = contiguousIndex(term, in: e.contentLower) {
                    let score = Double(term.utf16.count * 16 + (isBoundary(Array(e.contentLower.utf16), at) ? 10 : 0)) * 0.6
                    if score > termBest { termBest = score; termField = .content }
                }
                if termBest == -.infinity { ok = false; break }
                total += termBest
                if termBest > best { best = termBest; field = termField }
            }
            if !ok { continue }
            total += usageBoost(e, now: now.timeIntervalSince1970 * 1000)
            if !q.isEmpty, !e.snippet.isEnabled { total -= 15 }
            hits.append(SearchHit(snippet: e.snippet, score: total, field: field, nameHits: Set(positions).sorted()))
        }
        hits.sort { $0.score != $1.score ? $0.score > $1.score : $0.snippet.name.localizedCompare($1.snippet.name) == .orderedAscending }
        return Array(hits.prefix(max(0, limit)))
    }
    /// The palette's first `#group` term filters group names by prefix; remaining terms rank snippets.
    public func search(_ query: String, groups: [Group], limit: Int = 200, now: Date = Date()) -> [SearchHit] {
        guard let regex = try? NSRegularExpression(pattern: #"(?:^|\s)#(\S*)"#),
              let match = regex.firstMatch(in: query, range: NSRange(location: 0, length: query.utf16.count)),
              let whole = Range(match.range, in: query), let term = Range(match.range(at: 1), in: query) else {
            return search(query, limit: limit, now: now)
        }
        let prefix = query[term].lowercased()
        // `startsWith` compares UTF-16 code units, so the `e` of a decomposed `é` is a prefix.
        let ids = Set(groups.filter { $0.name.lowercased().utf16.starts(with: prefix.utf16) }.map(\.id))
        let filtered = entries.filter { $0.snippet.groupID.map { ids.contains($0) } ?? false }
        let text = ECMAScriptWhitespace.trim(query.replacingCharacters(in: whole, with: " "))
        return Self.search(filtered, query: text, limit: limit, now: now)
    }
    private static func contiguousIndex(_ term: String, in text: String) -> Int? {
        let t = Array(term.utf16), s = Array(text.utf16)
        guard t.count <= s.count else { return nil }
        return (0...(s.count - t.count)).first { s[$0..<($0 + t.count)].elementsEqual(t) }
    }
    private static func usageBoost(_ e: SearchEntry, now: Double) -> Double {
        let age = e.lastUsed == 0 ? Double.infinity : (now - e.lastUsed) / 86_400_000
        return log2(1 + Double(e.usage)) * 4 + (age < 1 ? 6 : age < 7 ? 3 : age < 30 ? 1 : 0)
    }
    private static func isBoundary(_ text: [UInt16], _ i: Int) -> Bool {
        if i == 0 { return true }
        let previous = text[i - 1]
        if "-_./:;,()[]{}'\"!?".utf16.contains(previous) || Unicode.Scalar(previous).map(ECMAScriptWhitespace.contains) == true { return true }
        return (97...122).contains(previous) && i < text.count && (48...57).contains(text[i])
    }
    /// Greedy subsequence matching with a backwards tightening pass (fzf model).
    public static func fuzzy(_ term: String, _ text: String, wantPositions: Bool = true) -> (score: Double, positions: [Int])? {
        let t = Array(term.utf16), s = Array(text.utf16), m = t.count, n = s.count
        if m == 0 { return (0, []) }; if m > n { return nil }
        if let start = (0...(n - m)).first(where: { Array(s[$0..<($0 + m)]) == t }) {
            let score = Double(m * 16 + 8 * (m - 1) + (isBoundary(s, start) ? 20 : 0) + (start == 0 ? 20 : 0) + (m == n ? 20 : 0)) - Double(min(start, 20)) * 0.5
            return (score, wantPositions ? Array(start..<(start + m)) : [])
        }
        var ti = 0, end = -1
        for i in 0..<n where s[i] == t[ti] { ti += 1; if ti == m { end = i; break } }
        if end == -1 { return nil }
        ti = m - 1; var start = end
        for i in stride(from: end, through: 0, by: -1) where s[i] == t[ti] { ti -= 1; if ti < 0 { start = i; break } }
        ti = 0; var score = 0.0, previous = -2, positions: [Int] = []
        for i in start...end {
            if ti >= m { break }
            if s[i] == t[ti] {
                score += Double(16 + (isBoundary(s, i) ? (ti == 0 ? 20 : 10) : 0) + (previous == i - 1 ? 4 : 0))
                previous = i; if wantPositions { positions.append(i) }; ti += 1
            } else { score -= previous == i - 1 ? 3 : 1 }
        }
        return (score, positions)
    }
}

public enum Search {
    public static func buildIndex(_ snippets: [Snippet], usage: [UUID: SnippetUsage] = [:]) -> [SearchEntry] { SearchIndex.buildIndex(snippets, usage: usage) }
    public static func search(_ entries: [SearchEntry], query: String, limit: Int = 200, now: Date = Date()) -> [SearchHit] { SearchIndex.search(entries, query: query, limit: limit, now: now) }
}
