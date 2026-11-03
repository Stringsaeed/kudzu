import AppKit
import KudzuCore
import Observation

/// One row in the palette's result list.
struct PaletteRow: Identifiable, Equatable {
    var snippet: SnippetRecord
    var nameHighlights: [Int]
    var groupPath: String?

    var id: UUID { snippet.id }
}

/// Search state for the palette: the library snapshot, the ranker, the query and the selection.
///
/// The snapshot is refreshed from `store.changes`. The ranker is rebuilt off the main actor,
/// since indexing thousands of snippets takes tens of milliseconds; ranking itself runs on every
/// keystroke and takes a few.
@MainActor @Observable
final class SearchPaletteModel {
    /// The palette shows at most this many results; more than a screenful is never useful.
    static let resultLimit = 200

    var query = "" {
        didSet {
            guard query != oldValue else { return }
            actionError = nil
            search()
            selectedIndex = 0
        }
    }

    private(set) var rows: [PaletteRow] = []
    private(set) var selectedIndex = 0
    /// Whether the library has any searchable snippet at all.
    private(set) var hasSnippets = false
    /// Non-nil when loading snippets from `store.library()` failed.
    private(set) var loadError: String?
    /// Non-nil when an explicit palette action (such as Insert) is blocked.
    var actionError: String?

    @ObservationIgnored private let store: KudzuStore
    @ObservationIgnored private var ranker = PaletteRanker()
    @ObservationIgnored private var snippetsByID: [UUID: SnippetRecord] = [:]
    @ObservationIgnored private var groupPaths: [UUID: String] = [:]
    /// Template sources by ID, for nested `{{snippet}}` tags in the preview.
    @ObservationIgnored private(set) var contents: [UUID: String] = [:]
    @ObservationIgnored private var isStale = true
    @ObservationIgnored private var hasLoaded = false
    @ObservationIgnored private var generation = 0
    @ObservationIgnored private var observation: Task<Void, Never>?
    /// The search field, so the panel can focus it.
    @ObservationIgnored weak var searchField: NSTextField?

    init(store: KudzuStore) {
        self.store = store
        let changes = store.changes
        observation = Task { [weak self] in
            for await _ in changes {
                guard let self else { return }
                self.isStale = true
                if self.isActive { self.refresh(synchronously: false) }
            }
        }
    }

    isolated deinit { observation?.cancel() }

    /// Set while the palette is on screen, so changes refresh it immediately.
    @ObservationIgnored var isActive = false

    var selectedRow: PaletteRow? {
        loadError == nil && rows.indices.contains(selectedIndex) ? rows[selectedIndex] : nil
    }

    /// Clears the query and makes sure the data is current.
    ///
    /// The first load is synchronous so the palette never opens empty. Later ones show the
    /// previous snapshot for the few milliseconds the rebuild takes.
    func prepareForShow() {
        actionError = nil
        if query.isEmpty { search() } else { query = "" }
        selectedIndex = 0
        if isStale { refresh(synchronously: !hasLoaded) }
    }

    /// Clears the search query while keeping keyboard focus inside the search field.
    func clearQuery() {
        query = ""
        focusSearchField()
    }

    /// Ensures the search field owns first-responder status inside its panel.
    func focusSearchField() {
        guard let field = searchField, let window = field.window else { return }
        if window.firstResponder !== field && window.firstResponder !== field.currentEditor() {
            window.makeFirstResponder(field)
        }
        if let editor = field.currentEditor(), (editor as? NSTextInputClient)?.hasMarkedText() != true {
            editor.selectedRange = NSRange(location: (field.stringValue as NSString).length, length: 0)
        }
    }

    func moveSelection(by delta: Int) {
        guard !rows.isEmpty else { return }
        actionError = nil
        selectedIndex = min(max(selectedIndex + delta, 0), rows.count - 1)
    }

    func select(_ id: UUID) {
        actionError = nil
        if let index = rows.firstIndex(where: { $0.id == id }) { selectedIndex = index }
    }

    // MARK: - Data

    func refreshNow() {
        refresh(synchronously: true)
    }

    private func refresh(synchronously: Bool) {
        let library: KudzuLibrary
        do {
            library = try store.library()
            loadError = nil
        } catch {
            loadError = error.localizedDescription
            return
        }
        isStale = false
        generation += 1
        let token = generation
        let summary = StatsCalculator.summary(usage: library.usage, snippets: library.snippets, typingSpeedWPM: 40)
        let stats = Dictionary(summary.perSnippet.map { ($0.snippetID, $0) }, uniquingKeysWith: { first, _ in first })

        if synchronously {
            apply(library, ranker: PaletteRanker(snippets: library.snippets, groups: library.groups, stats: stats))
        } else {
            Task { [weak self] in
                let ranker = await Self.buildRanker(library, stats)
                guard let self, self.generation == token else { return }
                self.apply(library, ranker: ranker)
            }
        }
    }

    @concurrent
    nonisolated private static func buildRanker(_ library: KudzuLibrary, _ stats: [UUID: SnippetStats]) async -> PaletteRanker {
        PaletteRanker(snippets: library.snippets, groups: library.groups, stats: stats)
    }

    private func apply(_ library: KudzuLibrary, ranker: PaletteRanker) {
        let selectedID = selectedRow?.id
        self.ranker = ranker
        snippetsByID = Dictionary(library.snippets.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
        contents = snippetsByID.mapValues(\.content)
        let tree = GroupTree(groups: library.groups)
        groupPaths = Dictionary(uniqueKeysWithValues: tree.allGroups.map { ($0.id, tree.path(to: $0.id).joined(separator: " / ")) })
        hasLoaded = true
        search()
        // Keep the user's place when the library changes under an open palette.
        if let selectedID, let index = rows.firstIndex(where: { $0.id == selectedID }) {
            selectedIndex = index
        } else {
            selectedIndex = min(selectedIndex, max(rows.count - 1, 0))
        }
    }

    private func search() {
        hasSnippets = ranker.count > 0
        rows = ranker.rank(query, limit: Self.resultLimit).compactMap { match in
            guard let snippet = snippetsByID[match.snippetID] else { return nil }
            return PaletteRow(snippet: snippet, nameHighlights: match.nameHighlights,
                              groupPath: snippet.groupID.flatMap { groupPaths[$0] })
        }
    }

    // MARK: - Rendering

    /// Renders a snippet as the palette shows and copies it: form fields take their default (or
    /// first option), and nested snippets resolve from the current library.
    ///
    /// The preview passes `placeholders: true`, so empty fields and the clipboard show as ‹name›
    /// instead of reading the real clipboard on every selection change.
    func render(_ snippet: SnippetRecord, placeholders: Bool) -> RenderResult {
        let contents = contents
        var formValues: [String: String] = [:]
        for field in TemplateAnalysis.formFields(in: snippet.content, rootSnippetID: snippet.id, resolveSnippet: { contents[$0] }) {
            formValues[field.name] = field.defaultValue ?? field.options.first ?? (placeholders ? "‹\(field.name)›" : "")
        }
        let context = RenderContext(
            now: .now,
            calendar: .current,
            timeZone: .current,
            locale: .current,
            clipboard: placeholders ? "‹clipboard›" : NSPasteboard.general.string(forType: .string),
            formValues: formValues,
            resolveSnippet: { contents[$0] },
            rootSnippetID: snippet.id
        )
        return TemplateRenderer.render(snippet.content, context: context)
    }

    /// Whether the snippet has form fields (the preview notes that defaults are shown).
    func hasFormFields(_ snippet: SnippetRecord) -> Bool {
        let contents = contents
        return !TemplateAnalysis.formFields(in: snippet.content, rootSnippetID: snippet.id, resolveSnippet: { contents[$0] }).isEmpty
    }
}
