// The quick-search tray's state (`palette.tsx` `Palette`): query, scope, `#group` filter, ranked
// results, keyboard selection, the ⌘-number overlay, the debounced live preview, and the action a
// key or click resolves to. Pure state plus one backend call (the preview render); the actions
// themselves go through `AppModel.perform(_:)` (`AppModel+Palette.swift`).

import Foundation
import KudzuCore
import Observation

/// The palette's scope chips, cycled with Tab / ⇧Tab.
public enum PaletteScope: String, CaseIterable, Hashable, Sendable {
    case all, favorites, recent

    public var label: String {
        switch self {
        case .all: "All"
        case .favorites: "Favorites"
        case .recent: "Recent"
        }
    }
}

/// What picking a row (or a shortcut) asks the backend to do.
public enum PaletteAction: Hashable, Sendable {
    /// ↵: hide the palette and insert into the app it opened over (`palette_insert`).
    case insert(UUID)
    /// ⌘↵: copy the rendered snippet (`palette_copy`).
    case copy(UUID)
    /// ⌘E: open the snippet in the main window (`show_main`).
    case edit(UUID)
    /// The "Create snippet “…”" row or ⌘N: a new snippet from the query (`palette_create`).
    case create(String)
    /// Esc with an empty query (`hide_palette`).
    case hide
}

/// A key the palette handles, decoded from an `NSEvent` by the view (``PaletteKeys``).
public enum PaletteKey: Hashable, Sendable {
    /// ↓ or ⌃N.
    case down
    /// ↑ or ⌃P.
    case up
    case pageDown, pageUp
    /// ↵, with ⌘ = copy.
    case enter(command: Bool)
    /// ⌘E.
    case edit
    /// ⌘N.
    case new
    /// ⌘1…⌘9 (1-based).
    case number(Int)
    /// Tab / ⇧Tab.
    case tab(shift: Bool)
    case escape
}

@MainActor @Observable
public final class PaletteModel {
    /// Results shown at most (`LIMIT`).
    public static let limit = 60
    /// Rows PageUp/PageDown move.
    public static let page = 7
    /// Delay before the preview renders the active row (the web's 40 ms timeout).
    public static let previewDelay: Duration = .milliseconds(40)

    // MARK: Inputs

    public private(set) var library: Library = .empty
    /// The app the palette opened over (`palette-show`'s `target`).
    public private(set) var target: AppInfo?
    /// Search text; editing it selects the first row.
    public var query = "" {
        didSet {
            guard query != oldValue else { return }
            active = 0
            refresh()
        }
    }
    public private(set) var scope: PaletteScope = .all
    /// Active row, possibly past the end (``clamped`` is what's drawn).
    public var active = 0 { didSet { if active != oldValue { previewIfNeeded() } } }
    /// ⌘ is held: the ⌘1–9 badges light up.
    public var showNumbers = false
    /// Replaces the footer's "Insert into …" after ⌘↵ ("Copied").
    public private(set) var flash: String?
    /// Bumped by every `palette-show`, so the view refocuses the field.
    public private(set) var showCount = 0
    /// Whether the library has loaded (`library === null` renders nothing in the list).
    public private(set) var loaded = false

    // MARK: Derived

    public private(set) var usage: [UUID: SnippetUsage] = [:]
    /// `Group` id → "Work › Clients".
    public private(set) var paths: [UUID: String] = [:]
    /// `#group` prefix (lowercased; "" while only `#` is typed), or nil.
    public private(set) var groupFilter: String?
    /// The query without the `#group` term.
    public private(set) var text = ""
    public private(set) var hits: [SearchHit] = []
    /// The rendered preview of the snippet with `id`.
    public private(set) var rendered: (id: UUID, result: RenderResult)?

    @ObservationIgnored private let backend: any Backend
    @ObservationIgnored private var index = SearchIndex(snippets: [])
    @ObservationIgnored private var previewTask: Task<Void, Never>?
    @ObservationIgnored private var previewed: Snippet?
    @ObservationIgnored private var eventTask: Task<Void, Never>?

    public init(backend: any Backend, library: Library? = nil) {
        self.backend = backend
        if let library { setLibrary(library) }
    }

    // MARK: Lifecycle

    /// Listens to `library` and `palette-show`, then loads the library (`api.getState`).
    public func start() async {
        let events = backend.events
        eventTask?.cancel()
        eventTask = Task { [weak self] in
            for await event in events {
                guard let self else { return }
                self.handle(event)
            }
        }
        if let state = try? await backend.getState() { setLibrary(state.library) }
    }

    public func stop() {
        eventTask?.cancel()
        eventTask = nil
        previewTask?.cancel()
    }

    public func handle(_ event: BackendEvent) {
        switch event {
        case let .library(library): setLibrary(library)
        case let .paletteShow(target): show(target: target)
        default: break
        }
    }

    /// `palette-show`: remember the target and start over (query cleared, All, first row).
    public func show(target: AppInfo?) {
        self.target = target
        flash = nil
        showNumbers = false
        scope = .all
        query = ""
        active = 0
        refresh()
        showCount += 1
    }

    /// A new library: re-folds only edited snippets (``SearchIndex/update``), recomputes paths.
    public func setLibrary(_ library: Library) {
        self.library = library
        loaded = true
        usage = Stats.usageById(library)
        index.update(snippets: library.snippets, usage: usage)
        var p: [UUID: String] = [:]
        for g in library.groups { p[g.id] = Groups.groupPath(library.groups, id: g.id).joined(separator: " › ") }
        paths = p
        refresh()
    }

    public func setScope(_ scope: PaletteScope) {
        self.scope = scope
        active = 0
        refresh()
    }

    // MARK: Search

    /// `"#work invoice"` → (`"invoice"`, `"work"`); the first `#term` only.
    public static func parse(_ query: String) -> (text: String, group: String?) {
        guard let regex = try? NSRegularExpression(pattern: #"(?:^|\s)#(\S*)"#),
              let m = regex.firstMatch(in: query, range: NSRange(location: 0, length: query.utf16.count)),
              let whole = Range(m.range, in: query), let term = Range(m.range(at: 1), in: query) else {
            return (query, nil)
        }
        let text = query.replacingCharacters(in: whole, with: " ").trimmingCharacters(in: .whitespacesAndNewlines)
        return (text, query[term].lowercased())
    }

    private func refresh() {
        let parsed = Self.parse(query)
        text = parsed.text
        groupFilter = parsed.group
        var entries = index.entries
        switch scope {
        case .all: break
        case .favorites: entries = entries.filter { $0.snippet.isFavorite }
        case .recent: entries = entries.filter { (usage[$0.snippet.id]?.last ?? 0) != 0 }
        }
        if let prefix = groupFilter {
            let ids = Set(library.groups.filter { $0.name.lowercased().utf16.starts(with: prefix.utf16) }.map(\.id))
            entries = entries.filter { $0.snippet.groupID.map(ids.contains) ?? false }
        }
        var r = SearchIndex.search(entries, query: text, limit: Self.limit)
        if text.isEmpty {
            if scope == .recent {
                r.sort { (usage[$0.snippet.id]?.last ?? 0) > (usage[$1.snippet.id]?.last ?? 0) }
            } else {
                // Favourites first, then score (a stable sort keeps the name tiebreak).
                r = r.enumerated().sorted { a, b in
                    if a.element.snippet.isFavorite != b.element.snippet.isFavorite { return a.element.snippet.isFavorite }
                    if a.element.score != b.element.score { return a.element.score > b.element.score }
                    return a.offset < b.offset
                }.map(\.element)
            }
        }
        hits = r
        previewIfNeeded()
    }

    // MARK: Rows

    public var trimmedQuery: String { query.trimmingCharacters(in: .whitespacesAndNewlines) }

    /// The "Create snippet “query”" row: a query that isn't exactly one of the results' triggers.
    public var createRow: Bool {
        let q = trimmedQuery
        guard !q.isEmpty else { return false }
        let lower = q.lowercased()
        return !hits.contains { $0.snippet.triggers.contains { $0.lowercased() == lower } }
    }

    public var total: Int { hits.count + (createRow ? 1 : 0) }
    public var clamped: Int { min(active, max(0, total - 1)) }
    public var current: Snippet? { clamped < hits.count ? hits[clamped].snippet : nil }

    /// The list's caption.
    public var sectionLabel: String {
        if !text.isEmpty { return "Best matches" }
        switch scope {
        case .recent: return "Recently used"
        case .favorites: return "Favorites"
        case .all: return "Suggested"
        }
    }

    /// The empty list's message.
    public var emptyMessage: String {
        switch scope {
        case .favorites: "No favorites yet."
        case .recent: "Nothing used yet."
        case .all: "No snippets yet."
        }
    }

    /// "Insert into Mail", or "Copied" after ⌘↵.
    public var footerLabel: String { flash ?? (target.map { "Insert into \($0.name)" } ?? "Insert") }

    /// The group path shown under a result.
    public func path(of snippet: Snippet) -> String { snippet.groupID.flatMap { paths[$0] } ?? "" }

    /// "3× · 2 hours ago" or "Never".
    public func usedLabel(_ snippet: Snippet, now: Date = Date()) -> String {
        guard let u = usage[snippet.id], u.count > 0 else { return "Never" }
        return "\(Stats.formatCount(u.count))× · \(Stats.relativeTime(u.last, now: now).lowercased())"
    }

    /// The finished preview of the active row, or nil while it renders.
    public var preview: RenderResult? {
        guard let current, let rendered, rendered.id == current.id else { return nil }
        return rendered.result
    }

    // MARK: Keys and picks

    /// Applies a key and returns the action it triggers, if any.
    @discardableResult
    public func press(_ key: PaletteKey) -> PaletteAction? {
        let n = max(1, total)
        switch key {
        case .down: active = (min(active, total - 1) + 1) % n
        case .up: active = (min(active, total - 1) - 1 + total) % n
        case .pageDown: active = max(0, min(total - 1, active + Self.page))
        case .pageUp: active = max(0, min(total - 1, active - Self.page))
        case let .enter(command): return total > 0 ? pick(clamped, mode: command ? .copy : .insert) : nil
        case .edit: return current != nil ? pick(clamped, mode: .edit) : nil
        case .new: return .create(trimmedQuery)
        case let .number(k):
            let i = k - 1
            return i >= 0 && i < hits.count ? pick(i) : nil
        case let .tab(shift):
            let all = PaletteScope.allCases
            let i = all.firstIndex(of: scope) ?? 0
            setScope(all[(i + (shift ? -1 : 1) + all.count) % all.count])
        case .escape:
            if query.isEmpty { return .hide }
            query = ""
            active = 0
        }
        return nil
    }

    public enum PickMode: Sendable { case insert, copy, edit }

    /// The action for row `i` (the create row past the hits); ⌘↵ also flashes "Copied".
    public func pick(_ i: Int, mode: PickMode = .insert) -> PaletteAction {
        guard i < hits.count else { return .create(trimmedQuery) }
        let id = hits[i].snippet.id
        switch mode {
        case .insert: return .insert(id)
        case .copy:
            flash = "Copied"
            return .copy(id)
        case .edit: return .edit(id)
        }
    }

    // MARK: Preview

    /// Renders the active row immediately. Typing still waits ``previewDelay``; previews call this
    /// so the first frame is not stuck on the shimmer.
    public func loadPreview() async {
        guard let snippet = current else { return }
        previewed = snippet
        previewTask?.cancel()
        previewTask = nil
        guard let result = try? await backend.renderPreview(content: snippet.content, snippetId: snippet.id, formValues: [:], formatting: snippet.formatting) else { return }
        guard current?.id == snippet.id else { return }
        rendered = (snippet.id, result)
    }

    /// Renders the active snippet 40 ms after it changes (`renderPreview` with no form values).
    private func previewIfNeeded() {
        let snippet = current
        guard snippet != previewed else { return }
        previewed = snippet
        previewTask?.cancel()
        guard let snippet else { return }
        previewTask = Task { [weak self, backend] in
            try? await Task.sleep(for: Self.previewDelay)
            guard !Task.isCancelled,
                  let result = try? await backend.renderPreview(content: snippet.content, snippetId: snippet.id, formValues: [:], formatting: snippet.formatting),
                  !Task.isCancelled else { return }
            self?.rendered = (snippet.id, result)
        }
    }
}
