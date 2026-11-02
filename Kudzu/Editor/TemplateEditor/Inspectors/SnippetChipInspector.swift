import KudzuCore
import KudzuUI
import SwiftUI

/// Snippet chips: a searchable picker over the other snippets.
struct SnippetChipInspector: View {
    let tag: KudzuCore.Tag
    let context: ChipInspectorContext
    let onChange: (KudzuCore.Tag) -> Void

    @State private var query = ""
    @State private var selection: UUID?
    @FocusState private var searchFocused: Bool

    init(tag: KudzuCore.Tag, context: ChipInspectorContext, onChange: @escaping (KudzuCore.Tag) -> Void) {
        self.tag = tag
        self.context = context
        self.onChange = onChange
        _selection = State(initialValue: tag.stringAttribute("id").flatMap(UUID.init(uuidString:)))
    }

    private var candidates: [SnippetRecord] {
        let others = context.snippets.filter { $0.id != context.currentSnippetID }
        let needle = query.trimmingCharacters(in: .whitespaces)
        guard !needle.isEmpty else { return others }
        return others.filter { snippet in
            snippet.name.localizedCaseInsensitiveContains(needle)
                || snippet.triggers.contains { $0.localizedCaseInsensitiveContains(needle) }
        }
    }

    var body: some View {
        InspectorStack {
            InspectorHeader(tag: tag, title: "Snippet", subtitle: "Inserts another snippet's content, expanded in place.")
            VStack(alignment: .leading, spacing: KzSpace.s2) {
                KzSearchField("Search by name or trigger", text: $query, shortcut: nil, focus: $searchFocused)
                    .onSubmit { if let first = candidates.first { pick(first.id) } }
                results
            }
        }
        .onAppear { searchFocused = true }
    }

    @ViewBuilder private var results: some View {
        if candidates.isEmpty {
            Text(context.snippets.count <= (context.currentSnippetID == nil ? 0 : 1)
                 ? "There are no other snippets yet." : "No snippets match “\(query)”.")
                .kzTextStyle(.callout, matchLineBox: false)
                .foregroundStyle(KzColor.inkMuted)
                .frame(maxWidth: .infinity, minHeight: 60)
        } else {
            let list = List(candidates, selection: Binding(get: { selection }, set: { if let id = $0 { pick(id) } })) { snippet in
                KzSnippetRow(name: snippet.name.isEmpty ? "Untitled snippet" : snippet.name, triggers: snippet.triggers,
                             isSelected: snippet.id == selection)
                    .kzListRow()
                    .tag(snippet.id)
            }
            list
                .kzSelectionList()
                .frame(height: min(CGFloat(candidates.count) * 46 + 8, 208))
        }
    }

    private func pick(_ id: UUID) {
        selection = id
        onChange(tag.setting("id", to: .string(id.uuidString)))
    }
}
