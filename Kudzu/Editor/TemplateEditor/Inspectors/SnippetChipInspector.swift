import KudzuCore
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
        VStack(alignment: .leading, spacing: 10) {
            InspectorHeader(tag: tag, title: "Snippet", subtitle: "Inserts another snippet's content, expanded in place.")
            TextField("Search snippets", text: $query, prompt: Text("Search by name or trigger"))
                .textFieldStyle(.roundedBorder)
                .focused($searchFocused)
                .onSubmit { if let first = candidates.first { pick(first.id) } }
            if candidates.isEmpty {
                Text(context.snippets.count <= (context.currentSnippetID == nil ? 0 : 1)
                     ? "There are no other snippets yet." : "No snippets match “\(query)”.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, minHeight: 60)
            } else {
                List(candidates, selection: Binding(get: { selection }, set: { if let id = $0 { pick(id) } })) { snippet in
                    VStack(alignment: .leading, spacing: 1) {
                        Text(snippet.name.isEmpty ? "Untitled snippet" : snippet.name)
                        if !snippet.triggers.isEmpty {
                            Text(snippet.triggers.joined(separator: "  "))
                                .font(.caption.monospaced())
                                .foregroundStyle(.secondary)
                        }
                    }
                    .tag(snippet.id)
                }
                .listStyle(.bordered)
                .frame(height: min(CGFloat(candidates.count) * 36 + 8, 200))
            }
        }
        .onAppear { searchFocused = true }
    }

    private func pick(_ id: UUID) {
        selection = id
        onChange(tag.setting("id", to: .string(id.uuidString)))
    }
}
