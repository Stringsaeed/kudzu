import Foundation

/// How the snippet list is ordered.
public enum SnippetSortOrder: String, CaseIterable, Sendable, Identifiable {
    case name
    case recentlyUsed
    case mostUsed
    case recentlyEdited

    public var id: String { rawValue }

    /// Menu title.
    public var title: String {
        switch self {
        case .name: "Name"
        case .recentlyUsed: "Recently Used"
        case .mostUsed: "Most Used"
        case .recentlyEdited: "Recently Edited"
        }
    }
}

/// Pre-folded text for fast, case- and diacritic-insensitive filtering of thousands of snippets.
///
/// Build once per library change (or patch one entry with `update`), then call `matches` per
/// keystroke: each check is a substring search on already-folded strings.
public struct SnippetSearchIndex: Sendable {
    private struct Entry: Sendable {
        var haystack: String
        var sortKey: String
    }

    private var entries: [UUID: Entry] = [:]

    /// An empty index.
    public init() {}

    /// Indexes every snippet.
    public init(snippets: [SnippetRecord]) {
        entries.reserveCapacity(snippets.count)
        for snippet in snippets { update(snippet) }
    }

    /// Adds or refreshes one snippet's entry.
    public mutating func update(_ snippet: SnippetRecord) {
        let haystack = Self.fold(([snippet.name] + snippet.triggers + [snippet.content]).joined(separator: "\n"))
        entries[snippet.id] = Entry(haystack: haystack, sortKey: Self.fold(snippet.name))
    }

    /// Drops a snippet's entry.
    public mutating func remove(_ id: UUID) {
        entries[id] = nil
    }

    /// Folds text for comparison: case, diacritics and width are ignored.
    public static func fold(_ text: String) -> String {
        text.folding(options: [.caseInsensitive, .diacriticInsensitive, .widthInsensitive], locale: nil)
    }

    /// Folded whitespace-separated terms; a snippet must contain all of them.
    public static func terms(from query: String) -> [String] {
        fold(query).split(whereSeparator: \.isWhitespace).map(String.init)
    }

    /// Whether the snippet's name, triggers or content contain every term. Empty terms match everything.
    public func matches(_ id: UUID, terms: [String]) -> Bool {
        guard !terms.isEmpty else { return true }
        guard let haystack = entries[id]?.haystack else { return false }
        return terms.allSatisfy { haystack.range(of: $0, options: .literal) != nil }
    }

    /// Snippets matching `query`, in their original order.
    public func filter(_ snippets: [SnippetRecord], query: String) -> [SnippetRecord] {
        let terms = Self.terms(from: query)
        guard !terms.isEmpty else { return snippets }
        return snippets.filter { matches($0.id, terms: terms) }
    }

    /// Orders snippets. Ties fall back to name, then id, so the order is stable between refreshes.
    ///
    /// Never-used snippets sort after used ones for `.recentlyUsed`.
    public func sorted(_ snippets: [SnippetRecord], by order: SnippetSortOrder, stats: [UUID: SnippetStats]) -> [SnippetRecord] {
        func key(_ s: SnippetRecord) -> String { entries[s.id]?.sortKey ?? Self.fold(s.name) }
        func byName(_ a: SnippetRecord, _ b: SnippetRecord) -> Bool {
            let ka = key(a), kb = key(b)
            return ka != kb ? ka < kb : a.id.uuidString < b.id.uuidString
        }
        switch order {
        case .name:
            return snippets.sorted(by: byName)
        case .recentlyEdited:
            return snippets.sorted { $0.updatedAt != $1.updatedAt ? $0.updatedAt > $1.updatedAt : byName($0, $1) }
        case .recentlyUsed:
            return snippets.sorted {
                let a = stats[$0.id]?.lastUsedAt ?? .distantPast, b = stats[$1.id]?.lastUsedAt ?? .distantPast
                return a != b ? a > b : byName($0, $1)
            }
        case .mostUsed:
            return snippets.sorted {
                let a = stats[$0.id]?.count ?? 0, b = stats[$1.id]?.count ?? 0
                return a != b ? a > b : byName($0, $1)
            }
        }
    }
}
