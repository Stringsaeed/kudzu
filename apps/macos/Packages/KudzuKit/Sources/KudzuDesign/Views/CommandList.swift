// `CommandList`: the ⌘K palette's filtered list (`command.tsx` over cmdk). A native `TextField` and
// `ScrollView`; the query filters and ranks items (fuzzy subsequence, contiguous and word-start
// matches score higher, like cmdk's command-score), groups keep their order, ↑/↓ move the
// highlight (no wrap), ↵ runs it, hovering highlights; "No results found." when nothing matches.

import SwiftUI

/// An action and its presentation in a command list.
public struct CommandItem: Identifiable {
    /// Stable identity for this value.
    public var id: String
    /// The user-visible title.
    public var title: String
    /// Optional leading SF Symbol.
    public var icon: Image?
    /// Optional keyboard shortcut combo.
    public var shortcut: String?
    /// Optional command grouping label.
    public var group: String?
    /// Additional searchable command terms.
    public var keywords: [String]
    /// Whether the command is unavailable.
    public var isDisabled: Bool
    /// Runs the command on the main actor.
    public var perform: @MainActor () -> Void

    /// Creates CommandItem with the supplied content and styling.
    public init(id: String, title: String, icon: Image? = nil, shortcut: String? = nil, group: String? = nil, keywords: [String] = [], isDisabled: Bool = false, perform: @escaping @MainActor () -> Void) {
        self.id = id
        self.title = title
        self.icon = icon
        self.shortcut = shortcut
        self.group = group
        self.keywords = keywords
        self.isDisabled = isDisabled
        self.perform = perform
    }
}

/// Fuzzy ranking for command-list actions.
public enum CommandScore {
    /// 0 when `query` is not a subsequence of `text`; otherwise higher for tighter, earlier,
    /// word-start matches (1 for an exact match).
    public static func score(_ text: String, _ query: String) -> Double {
        let q = Array(query.lowercased().filter { !$0.isWhitespace })
        guard !q.isEmpty else { return 1 }
        let t = Array(text.lowercased())
        if text.lowercased() == query.lowercased() { return 1 }
        var score = 0.0
        var ti = 0
        var last = -1
        for c in q {
            guard let found = t[ti...].firstIndex(of: c) else { return 0 }
            var s = 0.1
            if found == last + 1 { s = 0.9 } else if found == 0 || t[found - 1] == " " || t[found - 1] == "-" { s = 0.8 }
            score += s
            last = found
            ti = found + 1
        }
        return min(0.99, score / Double(q.count) * (0.9 + 0.1 * Double(q.count) / Double(max(t.count, 1))))
    }
}

/// A searchable command list composed from native fields, buttons and scrolling.
public struct CommandList: View {
    @Binding var query: String
    var items: [CommandItem]
    var placeholder: String
    var emptyText: String
    var onRun: (() -> Void)?

    @State private var selected: String?
    @FocusState private var focused: Bool
    @Environment(\.colorScheme) private var scheme

    /// Creates CommandList with the supplied content and styling.
    public init(query: Binding<String>, items: [CommandItem], placeholder: String = "Type a command or search…", emptyText: String = "No results found.", onRun: (() -> Void)? = nil) {
        _query = query
        self.items = items
        self.placeholder = placeholder
        self.emptyText = emptyText
        self.onRun = onRun
    }

    /// Visible items, grouped, ranked by score when there is a query.
    var groups: [(name: String?, items: [CommandItem])] {
        let scored = items.compactMap { item -> (CommandItem, Double)? in
            let s = ([item.title] + item.keywords).map { CommandScore.score($0, query) }.max() ?? 0
            return s > 0 ? (item, s) : nil
        }
        var order: [String?] = []
        var buckets: [String?: [(CommandItem, Double)]] = [:]
        for (item, s) in scored {
            if buckets[item.group] == nil { order.append(item.group) }
            buckets[item.group, default: []].append((item, s))
        }
        let ranked = order.map { g -> (String?, [CommandItem], Double) in
            let list = buckets[g] ?? []
            let sorted = query.isEmpty ? list : list.sorted { $0.1 > $1.1 }
            return (g, sorted.map(\.0), sorted.map(\.1).max() ?? 0)
        }
        let groups = query.isEmpty ? ranked : ranked.sorted { $0.2 > $1.2 }
        return groups.map { ($0.0, $0.1) }
    }

    var flat: [CommandItem] { groups.flatMap(\.items).filter { !$0.isDisabled } }

    /// The composed native view.
    public var body: some View {
        let t = KZTheme(scheme)
        let groups = groups
        VStack(spacing: 0) {
            InputGroup {
                Icon.search.icon(16).opacity(0.5)
            } field: {
                TextField(placeholder, text: $query)
                    .focused($focused)
                    .onKeyPress(.downArrow) { move(1); return .handled }
                    .onKeyPress(.upArrow) { move(-1); return .handled }
                    .onKeyPress(.return) { run(); return .handled }
            }
            .padding([.horizontal, .top], 4)

            ScrollViewReader { proxy in
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        if groups.isEmpty {
                            Text(emptyText).font(.system(size: 14)).frame(maxWidth: .infinity).padding(.vertical, 24)
                        }
                        ForEach(Array(groups.enumerated()), id: \.offset) { index, group in
                            if index > 0 { Divider().padding(.horizontal, -4) }
                            VStack(alignment: .leading, spacing: 0) {
                                if let name = group.name {
                                    Text(name).font(.system(size: 12, weight: .medium)).mutedForeground()
                                        .padding(.horizontal, 8).padding(.vertical, 6)
                                }
                                ForEach(group.items) { item in
                                    row(item, t: t).id(item.id)
                                }
                            }
                            .padding(4)
                        }
                    }
                }
                .frame(maxHeight: 288)
                .onChange(of: selected) { _, id in if let id { proxy.scrollTo(id) } }
            }
        }
        .padding(4)
        .background(t[.popover], in: RoundedRectangle(cornerRadius: Radius.xl))
        .foregroundStyle(t[.popoverForeground])
        .onAppear { focused = true; selected = flat.first?.id }
        .onChange(of: query) { selected = flat.first?.id }
    }

    private func row(_ item: CommandItem, t: KZTheme) -> some View {
        let on = selected == item.id
        return Button {
            selected = item.id
            run()
        } label: {
            HStack(spacing: 8) {
            if let icon = item.icon { icon.icon(16).foregroundStyle(on ? t[.foreground] : t[.mutedForeground]) }
            Text(item.title).font(.system(size: 14)).lineLimit(1)
            Spacer(minLength: 0)
            if let shortcut = item.shortcut {
                Shortcut(shortcut, tone: .ghost)
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
        .background(on ? t[.muted] : .clear, in: RoundedRectangle(cornerRadius: Radius.sm))
        .foregroundStyle(on ? t[.foreground] : t[.popoverForeground])
        .opacity(item.isDisabled ? 0.5 : 1)
        .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(item.isDisabled)
        .onHover { if $0 && !item.isDisabled { selected = item.id } }
    }

    private func move(_ d: Int) {
        let list = flat
        guard !list.isEmpty else { return }
        let i = list.firstIndex { $0.id == selected } ?? -1
        selected = list[min(max(i + d, 0), list.count - 1)].id
    }

    private func run() {
        guard let item = flat.first(where: { $0.id == selected }) else { return }
        item.perform()
        onRun?()
    }
}
