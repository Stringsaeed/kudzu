import AppKit
import KudzuCore
import SwiftUI

/// Content of the search palette: search field, results, preview and key hints.
struct SearchPaletteView: View {
    let model: SearchPaletteModel
    let controller: SearchPaletteController

    var body: some View {
        VStack(spacing: 0) {
            searchBar
            Divider()
            if model.rows.isEmpty {
                emptyState
            } else {
                HStack(spacing: 0) {
                    PaletteResultsList(model: model, controller: controller)
                        .frame(width: 340)
                    Divider()
                    if let row = model.selectedRow {
                        PalettePreview(model: model, snippet: row.snippet, groupPath: row.groupPath)
                    } else {
                        Spacer().frame(maxWidth: .infinity)
                    }
                }
            }
            Divider()
            PaletteHintBar()
        }
        .frame(width: SearchPaletteController.panelSize.width, height: SearchPaletteController.panelSize.height)
    }

    private var searchBar: some View {
        HStack(spacing: 12) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 20, weight: .medium))
                .foregroundStyle(.secondary)
                .accessibilityHidden(true)
            PaletteSearchField(model: model)
        }
        .padding(.horizontal, 18)
        .frame(height: 58)
    }

    private var emptyState: some View {
        VStack(spacing: 8) {
            Image(systemName: model.hasSnippets ? "text.magnifyingglass" : "text.badge.plus")
                .font(.system(size: 34, weight: .light))
                .foregroundStyle(.tertiary)
            Text(model.hasSnippets ? "No Matching Snippets" : "No Snippets Yet")
                .font(.headline)
                .foregroundStyle(.secondary)
            Text(model.hasSnippets ? "Try fewer letters, or search by trigger or content." : "Create snippets in the Kudzu window.")
                .font(.callout)
                .foregroundStyle(.tertiary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Search field

/// A large borderless text field; AppKit so the panel can focus it the moment it opens.
private struct PaletteSearchField: NSViewRepresentable {
    let model: SearchPaletteModel

    func makeNSView(context: Context) -> NSTextField {
        let field = FocusingTextField()
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.font = .systemFont(ofSize: 22, weight: .regular)
        field.placeholderString = "Search snippets"
        field.lineBreakMode = .byTruncatingTail
        field.usesSingleLineMode = true
        field.cell?.isScrollable = true
        field.cell?.wraps = false
        field.delegate = context.coordinator
        field.setAccessibilityLabel("Search snippets")
        model.searchField = field
        return field
    }

    func updateNSView(_ field: NSTextField, context: Context) {
        if field.stringValue != model.query { field.stringValue = model.query }
    }

    func makeCoordinator() -> Coordinator { Coordinator(model: model) }

    /// Takes focus as soon as it lands in the panel, which the first `show()` can't do because
    /// SwiftUI builds the view after the panel is ordered in.
    final class FocusingTextField: NSTextField {
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            window?.makeFirstResponder(self)
        }
    }

    @MainActor
    final class Coordinator: NSObject, NSTextFieldDelegate {
        let model: SearchPaletteModel

        init(model: SearchPaletteModel) { self.model = model }

        func controlTextDidChange(_ notification: Notification) {
            guard let field = notification.object as? NSTextField else { return }
            model.query = field.stringValue
        }
    }
}

// MARK: - Results

private struct PaletteResultsList: View {
    let model: SearchPaletteModel
    let controller: SearchPaletteController

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 2) {
                    ForEach(Array(model.rows.enumerated()), id: \.element.id) { index, row in
                        PaletteResultRow(row: row, isSelected: index == model.selectedIndex)
                            .id(row.id)
                            .contentShape(Rectangle())
                            .onTapGesture {
                                // One gesture, so a single click selects without waiting out the
                                // double-click interval; the second click of a double-click inserts.
                                model.select(row.id)
                                if NSApp.currentEvent?.clickCount == 2 { controller.insertSelection() }
                            }
                    }
                }
                .padding(6)
            }
            .scrollIndicators(.automatic)
            .onChange(of: model.selectedIndex) { _, index in
                guard model.rows.indices.contains(index) else { return }
                proxy.scrollTo(model.rows[index].id)
            }
            .onChange(of: model.query) {
                if let first = model.rows.first { proxy.scrollTo(first.id, anchor: .top) }
            }
        }
    }
}

private struct PaletteResultRow: View {
    let row: PaletteRow
    let isSelected: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: 6) {
                Text(Self.highlighted(row.snippet.name, row.nameHighlights))
                    .font(.system(size: 13, weight: .medium))
                    .lineLimit(1)
                if row.snippet.isFavorite {
                    Image(systemName: "star.fill")
                        .font(.system(size: 9))
                        .foregroundStyle(.yellow)
                        .accessibilityLabel("Favorite")
                }
                Spacer(minLength: 6)
                if let groupPath = row.groupPath {
                    Text(groupPath)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.head)
                }
            }
            HStack(spacing: 4) {
                ForEach(Array(row.snippet.triggers.prefix(2).enumerated()), id: \.offset) { _, trigger in
                    TriggerChip(trigger: trigger)
                }
                if row.snippet.triggers.count > 2 {
                    Text("+\(row.snippet.triggers.count - 2)")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                Text(Self.excerpt(row.snippet.content))
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background {
            if isSelected {
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(Color.accentColor.opacity(0.22))
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }

    /// The name with matched characters in bold accent.
    static func highlighted(_ name: String, _ offsets: [Int]) -> AttributedString {
        guard !offsets.isEmpty else { return AttributedString(name) }
        let matched = Set(offsets)
        var result = AttributedString()
        var run = ""
        var runMatched = false
        func flush() {
            guard !run.isEmpty else { return }
            var part = AttributedString(run)
            if runMatched {
                part.foregroundColor = .accentColor
                part.font = .system(size: 13, weight: .bold)
            }
            result += part
            run = ""
        }
        for (offset, character) in name.enumerated() {
            let isMatch = matched.contains(offset)
            if isMatch != runMatched { flush(); runMatched = isMatch }
            run.append(character)
        }
        flush()
        return result
    }

    /// The first line's worth of raw content, whitespace collapsed.
    static func excerpt(_ content: String) -> String {
        content.prefix(160).split(whereSeparator: \.isWhitespace).joined(separator: " ")
    }
}

private struct TriggerChip: View {
    let trigger: String

    var body: some View {
        Text(trigger)
            .font(.system(size: 10.5, weight: .medium, design: .monospaced))
            .foregroundStyle(.secondary)
            .lineLimit(1)
            .padding(.horizontal, 5)
            .padding(.vertical, 1)
            .background(.quaternary, in: RoundedRectangle(cornerRadius: 4, style: .continuous))
    }
}

// MARK: - Preview

private struct PalettePreview: View {
    let model: SearchPaletteModel
    let snippet: SnippetRecord
    let groupPath: String?

    var body: some View {
        let result = model.render(snippet, placeholders: true)
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 6) {
                Text(snippet.name)
                    .font(.headline)
                    .lineLimit(2)
                HStack(spacing: 4) {
                    ForEach(Array(snippet.triggers.enumerated()), id: \.offset) { _, trigger in
                        TriggerChip(trigger: trigger)
                    }
                    Spacer(minLength: 6)
                    if let groupPath {
                        Label(groupPath, systemImage: "folder")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                }
            }
            .padding(14)

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    if result.text.isEmpty {
                        Text("This snippet is empty.")
                            .foregroundStyle(.tertiary)
                    } else {
                        Text(SnippetPreviewView.attributed(result))
                            .font(.body)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    if model.hasFormFields(snippet) {
                        Label("Form fields show their default values.", systemImage: "list.bullet.rectangle")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                .padding(14)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

// MARK: - Hints

private struct PaletteHintBar: View {
    var body: some View {
        HStack(spacing: 14) {
            Image(systemName: "text.badge.plus")
                .foregroundStyle(.tertiary)
                .accessibilityHidden(true)
            Spacer()
            Hint(keys: "↵", title: "Insert")
            Hint(keys: "⌘↵", title: "Copy")
            Hint(keys: "⌘E", title: "Edit")
        }
        .font(.caption)
        .padding(.horizontal, 14)
        .frame(height: 32)
    }

    private struct Hint: View {
        let keys: String
        let title: String

        var body: some View {
            HStack(spacing: 5) {
                Text(title)
                    .foregroundStyle(.secondary)
                Text(keys)
                    .font(.system(size: 11, weight: .semibold, design: .rounded))
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 5)
                    .padding(.vertical, 1)
                    .background(.quaternary, in: RoundedRectangle(cornerRadius: 4, style: .continuous))
            }
            .accessibilityElement(children: .combine)
        }
    }
}
