// The searchable list inside the editor's popovers (`Command` + `CommandInput` + `CommandList` in
// `tag-tools.tsx` and `app-picker.tsx`): a native search field, rows filtered and ranked with
// cmdk's scoring (`CommandScore`), groups in order, ↑/↓ moving the highlight with wrap-around
// (`loop`), ↵ choosing, hover highlighting, and an empty state. KudzuDesign's `CommandList` draws
// fixed rows; these popovers need two-line rows, trailing chips and check marks.

import KudzuDesign
import SwiftUI

struct PickItem: Identifiable {
    var id: String
    /// The text the query matches (cmdk's `value`: label plus keywords).
    var value: String
    var group: String?
    var perform: @MainActor () -> Void
}

struct PickList<Row: View, Empty: View>: View {
    @Binding var query: String
    var placeholder: String
    var items: [PickItem]
    var maxHeight: CGFloat = 288
    /// The row highlighted first (the current value), else the first row.
    var initial: String?
    @ViewBuilder var row: (PickItem, Bool) -> Row
    @ViewBuilder var empty: () -> Empty

    @State private var highlighted: String?
    @FocusState private var focused: Bool

    /// Visible rows, grouped; ranked by score when there is a query.
    static func filter(_ items: [PickItem], query: String) -> [(name: String?, items: [PickItem])] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        var order: [String?] = []
        var buckets: [String?: [(PickItem, Double)]] = [:]
        for item in items {
            let score = trimmed.isEmpty ? 1 : CommandScore.score(item.value, trimmed)
            guard score > 0 else { continue }
            if buckets[item.group] == nil { order.append(item.group) }
            buckets[item.group, default: []].append((item, score))
        }
        let ranked = order.map { group -> (String?, [PickItem], Double) in
            let list = buckets[group] ?? []
            let sorted = trimmed.isEmpty ? list : list.enumerated().sorted { a, b in
                a.element.1 != b.element.1 ? a.element.1 > b.element.1 : a.offset < b.offset
            }.map(\.element)
            return (group, sorted.map(\.0), sorted.map(\.1).max() ?? 0)
        }
        let groups = trimmed.isEmpty ? ranked : ranked.enumerated().sorted { a, b in
            a.element.2 != b.element.2 ? a.element.2 > b.element.2 : a.offset < b.offset
        }.map(\.element)
        return groups.map { ($0.0, $0.1) }
    }

    var body: some View {
        let groups = Self.filter(items, query: query)
        let flat = groups.flatMap(\.items)
        VStack(spacing: 0) {
            // `CommandInput`: `p-1 pb-0` around an `h-8 rounded-lg bg-input/30` well. No hairline,
            // and no dividers between groups — cmdk's groups are just padded stacks.
            HStack(spacing: 8) {
                Icon.search.icon(16).foregroundStyle(Color.kz.mutedForeground).opacity(0.5)
                TextField(text: $query, prompt: Text(placeholder).foregroundStyle(Color.kz.mutedForeground)) {
                    Text(placeholder)
                }
                .textFieldStyle(.plain)
                .font(.system(size: 14))
                .focused($focused)
                .onKeyPress(.downArrow) { move(1, in: flat); return .handled }
                .onKeyPress(.upArrow) { move(-1, in: flat); return .handled }
                .onKeyPress(.return) { choose(in: flat); return .handled }
            }
            .padding(.horizontal, 8)
            .frame(height: 32)
            .background {
                RoundedRectangle(cornerRadius: Radius.lg)
                    .fill(Color.kz.input.opacity(0.3))
            }
            .overlay {
                RoundedRectangle(cornerRadius: Radius.lg)
                    .strokeBorder(focused ? Color.kz.ring.opacity(0.5) : Color.kz.input.opacity(0.3), lineWidth: focused ? 3 : 1)
            }
            .padding(.horizontal, 4)
            .padding(.top, 4)

            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        if groups.isEmpty {
                            empty()
                                .font(.system(size: 14))
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 24)
                        }
                        ForEach(Array(groups.enumerated()), id: \.offset) { _, group in
                            VStack(alignment: .leading, spacing: 0) {
                                if let name = group.name {
                                    Text(name)
                                        .font(.system(size: 12, weight: .medium))
                                        .foregroundStyle(Color.kz.mutedForeground)
                                        .padding(.horizontal, 8)
                                        .padding(.vertical, 6)
                                }
                                ForEach(group.items) { item in
                                    rowView(item, on: highlighted == item.id).id(item.id)
                                }
                            }
                            .padding(4)
                        }
                    }
                }
                .frame(maxHeight: maxHeight)
                .fixedSize(horizontal: false, vertical: true)
                .onChange(of: highlighted) { _, id in if let id { proxy.scrollTo(id) } }
            }
        }
        .onAppear {
            focused = true
            highlighted = initial.flatMap { id in flat.contains { $0.id == id } ? id : nil } ?? flat.first?.id
        }
        .onChange(of: query) { highlighted = Self.filter(items, query: query).first?.items.first?.id }
    }

    private func rowView(_ item: PickItem, on: Bool) -> some View {
        row(item, on)
            .font(.system(size: 14))
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(on ? Color.kz.muted : .clear, in: RoundedRectangle(cornerRadius: Radius.sm))
            .contentShape(Rectangle())
            .onHover { if $0 { highlighted = item.id } }
            .onTapGesture { highlighted = item.id; item.perform() }
            .accessibilityElement(children: .combine)
            .accessibilityAddTraits(.isButton)
            .accessibilityAction { item.perform() }
    }

    /// ↑/↓ with wrap-around (`<Command loop>`).
    private func move(_ delta: Int, in flat: [PickItem]) {
        guard !flat.isEmpty else { return }
        let i = flat.firstIndex { $0.id == highlighted } ?? (delta > 0 ? -1 : 0)
        highlighted = flat[(i + delta + flat.count) % flat.count].id
    }

    private func choose(in flat: [PickItem]) {
        flat.first { $0.id == highlighted }?.perform()
    }
}
