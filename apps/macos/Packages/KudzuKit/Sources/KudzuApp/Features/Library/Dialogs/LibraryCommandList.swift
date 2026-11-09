// Feature-local rendering for command-menu.tsx's already-filtered results. The shared
// CommandList applies fuzzy filtering again and cannot display snippet paths or triggers.

import KudzuDesign
import SwiftUI

struct LibraryCommandList: View {
    @Binding var query: String
    var rows: [LibraryCommandRow]
    var icon: (LibraryCommandRow) -> Image
    var run: (LibraryCommandRow) -> Void
    @State private var selected: String?
    @FocusState private var searchFocused: Bool

    private var headings: [String] {
        rows.reduce(into: []) { headings, row in
            if headings.last != row.heading { headings.append(row.heading) }
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Icon.search.icon(16).foregroundStyle(Color.kz.mutedForeground)
                TextField(text: $query, prompt: Text("Search snippets, groups and actions…").foregroundStyle(Color.kz.faint)) {
                    Text("Search snippets, groups and actions")
                }
                .textFieldStyle(.plain)
                .font(.system(size: 15))
                .focused($searchFocused)
                .onKeyPress(.downArrow) { move(1); return .handled }
                .onKeyPress(.upArrow) { move(-1); return .handled }
                .onSubmit(choose)
                Shortcut("escape")
            }
            .padding(.horizontal, 16)
            .frame(height: 48)
            Hairline()
            if rows.isEmpty {
                Text("No results for “\(query)”.")
                    .font(.system(size: 14))
                    .foregroundStyle(Color.kz.mutedForeground)
                    .padding(.vertical, 40)
                    .frame(maxWidth: .infinity)
            } else {
                ScrollViewReader { proxy in
                    // `List px-2 pb-2`; headings are plain rows (`px-2.5 pt-3 pb-1 text-[11px]
                    // font-medium text-faint`) rather than the system's sticky section headers.
                    List(selection: $selected) {
                        ForEach(headings, id: \.self) { heading in
                            Text(heading)
                                .font(.system(size: 11, weight: .medium))
                                .foregroundStyle(Color.kz.faint)
                                .padding(.bottom, 4)
                                .frame(maxWidth: .infinity, minHeight: Self.headingHeight, maxHeight: Self.headingHeight, alignment: .bottomLeading)
                                .selectionDisabled()
                                .listRowInsets(Self.rowInsets)
                                .listRowSeparator(.hidden)
                                .kzOwnSelection()
                            ForEach(rows.filter { $0.heading == heading }) { row in
                                command(row)
                                    .tag(row.id)
                                    .id(row.id)
                                    .listRowInsets(Self.rowInsets)
                                    .listRowSeparator(.hidden)
                                    .listRowBackground(rowFill(selected == row.id))
                            }
                        }
                    }
                    .listStyle(.plain)
                    .scrollContentBackground(.hidden)
                    .environment(\.defaultMinListRowHeight, 0)
                    .contentMargins(.bottom, 8, for: .scrollContent)
                    .frame(height: min(440, CGFloat(rows.count) * Self.rowHeight + CGFloat(headings.count) * Self.headingHeight + 8))
                    .onKeyPress(.downArrow) { move(1); return .handled }
                    .onKeyPress(.upArrow) { move(-1); return .handled }
                    .onKeyPress(.return) { choose(); return .handled }
                    .onChange(of: selected) { _, id in
                        if let id { proxy.scrollTo(id) }
                    }
                }
            }
        }
        .background(Color.kz.popover)
        .onAppear { searchFocused = true; selected = rows.first?.id }
        .onChange(of: query) { _, _ in selected = rows.first?.id }
        .onChange(of: rows.map(\.id)) { _, ids in
            if selected.map(ids.contains) != true { selected = ids.first }
        }
    }

    private static let rowHeight: CGFloat = 36
    private static let headingHeight: CGFloat = 32
    /// The list's 8 pt inset plus the row's own 10 pt padding.
    private static let rowInsets = EdgeInsets(top: 0, leading: 18, bottom: 0, trailing: 18)

    /// `rounded-lg data-[selected=true]:bg-foreground/[0.07]`, inside the list's 8 pt inset.
    private func rowFill(_ on: Bool) -> some View {
        RoundedRectangle(cornerRadius: Radius.lg)
            .fill(on ? Color.kz.mix(.foreground, alpha: 0.07) : .clear)
            .padding(.horizontal, 8)
    }

    /// `flex h-9 items-center gap-2.5 text-[13px] text-foreground/90`, full foreground when selected.
    private func command(_ row: LibraryCommandRow) -> some View {
        let on = selected == row.id
        return Button { run(row) } label: {
            HStack(spacing: 10) {
                icon(row).icon(16).foregroundStyle(Color.kz.mutedForeground)
                Text(row.title)
                    .foregroundStyle(row.id == "del" ? Color.kz.destructive : Color.kz.mix(.foreground, alpha: on ? 1 : 0.9))
                    .lineLimit(1)
                if let path = row.groupPath, !path.isEmpty {
                    Text(path).font(.system(size: 12)).foregroundStyle(Color.kz.faint).lineLimit(1)
                }
                Spacer(minLength: 0)
                if let trigger = row.trigger, !trigger.isEmpty { TriggerChip(trigger) }
                if let shortcut = row.shortcut { Shortcut(shortcut) }
            }
            .font(.system(size: 13))
            .frame(height: Self.rowHeight)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { if $0 { selected = row.id } }
        .kzOwnSelection()
    }

    private func move(_ delta: Int) {
        selected = LibraryCommandRows.moved(selected, by: delta, rows: rows)
    }

    private func choose() {
        guard let row = rows.first(where: { $0.id == selected }) else { return }
        run(row)
    }
}
