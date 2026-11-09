import Foundation
import KudzuCore

// The loaded library: lookups, inherited settings and rendering helpers.
// Port of `kudzu-engine/src/library.rs` (Rust `Library`; renamed because KudzuCore's `Library`
// is the whole-library interchange model).

/// An immutable snapshot of what the engine matches and renders against.
struct LibrarySnapshot: Hashable, Sendable {
    /// Nested snippet depth limit, matching the renderer.
    static let maxDepth = 8

    var snippets: [UUID: Snippet] = [:]
    var groups: [Group] = []
    var config = MatchingConfig()
    var locale = ""
    /// Swift model equality normalizes Unicode; Rust compares the stored bytes. Keep an exact,
    /// deterministic record representation so normalization-only edits still reload the engine.
    private var recordBytes: [Data]? = []

    init() {}

    /// Snapshot of `library`; duplicate snippet ids keep the first record. An empty locale uses `fallbackLocale`.
    init(_ library: EngineLibrary, fallbackLocale: () -> String) {
        var snippets: [UUID: Snippet] = [:]
        snippets.reserveCapacity(library.snippets.count)
        for snippet in library.snippets where snippets[snippet.id] == nil {
            snippets[snippet.id] = snippet
        }
        self.snippets = snippets
        groups = library.groups
        config = library.config
        locale = library.locale.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? fallbackLocale() : library.locale
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        recordBytes = try? [
            encoder.encode(snippets.values.sorted { $0.id.uuidString < $1.id.uuidString }),
            encoder.encode(groups),
            Data(locale.utf8),
        ]
    }

    /// Exact no-op reload check. If encoding fails, rebuild rather than keep stale records.
    func hasSameContents(as other: LibrarySnapshot) -> Bool {
        recordBytes != nil && self == other
    }

    /// Template source of snippet `id`.
    func source(_ id: UUID) -> String? {
        snippets[id]?.content
    }

    /// Form fields of `snippet`, including nested snippets.
    func formFields(_ snippet: Snippet) -> [FormFieldSpec] {
        Template.formFields(snippet.content, resolveSnippet: { source($0) })
    }

    /// Whether `snippet` (or a nested snippet) reads the clipboard; avoids touching it otherwise.
    func usesClipboard(_ snippet: Snippet) -> Bool {
        var visited: Set<UUID> = [snippet.id]
        return sourceUsesClipboard(snippet.content, visited: &visited, depth: 0)
    }

    private func sourceUsesClipboard(_ source: String, visited: inout Set<UUID>, depth: Int) -> Bool {
        for case .tag(let tag) in Template.parse(source).nodes {
            switch Template.typed(tag) {
            case .clipboard:
                return true
            case .snippet(let id) where depth < Self.maxDepth && visited.insert(id).inserted:
                if let nested = snippets[id], sourceUsesClipboard(nested.content, visited: &visited, depth: depth + 1) {
                    return true
                }
            default:
                break
            }
        }
        return false
    }

    /// Renders `snippet` at `now` (in `timeZone`) with the given clipboard snapshot and form values.
    func render(
        _ snippet: Snippet,
        now: Date = Date(),
        timeZone: TimeZone = .current,
        clipboard: String?,
        formValues: [String: String]
    ) -> RenderResult {
        let all = snippets
        let context = RenderContext(
            now: now,
            timeZone: timeZone,
            locale: Locale(identifier: locale),
            clipboard: clipboard,
            formValues: formValues,
            resolveSnippet: { all[$0]?.content },
            resolveFormatting: { all[$0]?.formatting },
            rootSnippetID: snippet.id
        )
        return Template.renderFormatted(snippet.content, formatting: snippet.formatting, context: context)
    }

    /// The snippet's own rule and insertion, else the nearest ancestor group's (cycle-safe).
    private func inherited(_ snippet: Snippet) -> (rule: AppRule?, insertion: InsertionMethod?) {
        var rule = snippet.rule
        var insertion = snippet.insertion
        var visited = Set<UUID>()
        var next = snippet.groupID
        while let id = next {
            if (rule != nil && insertion != nil) || !visited.insert(id).inserted {
                break
            }
            guard let group = groups.first(where: { $0.id == id }) else { break }
            if rule == nil {
                rule = group.rule
            }
            if insertion == nil {
                insertion = group.insertion
            }
            next = group.parentID
        }
        return (rule, insertion)
    }

    /// Insertion method for `snippet` in `bundleID`: rule's per-app override ▸ snippet/group ▸
    /// global per-app ▸ global default.
    func insertion(for snippet: Snippet, bundleID: String?) -> InsertionMethod {
        let (rule, insertion) = inherited(snippet)
        if let rule, let bundleID, let method = rule.insertion, rule.bundleIDs.contains(bundleID) {
            return method
        }
        if let insertion {
            return insertion
        }
        if let bundleID, let method = config.appInsertions[bundleID] {
            return method
        }
        return config.defaultInsertion
    }

    /// Whether nothing may ever expand in `bundleID` (password managers, login window…).
    func isDenied(_ bundleID: String?) -> Bool {
        bundleID.map { config.deniedBundleIDs.contains($0) } ?? false
    }

    /// `values` completed with each field's default, else its first choice, else empty text.
    static func withDefaults(_ fields: [FormFieldSpec], _ values: [String: String]) -> [String: String] {
        var values = values
        for field in fields where values[field.name] == nil {
            values[field.name] = field.defaultValue ?? field.options.first ?? ""
        }
        return values
    }
}
