import AppKit
import KudzuCore
import KudzuUI
import SwiftUI

/// Content of the search palette: search field, results, preview and key hints.
struct SearchPaletteView: View {
    let model: SearchPaletteModel
    var controller: SearchPaletteController?

    var body: some View {
        VStack(spacing: 0) {
            searchBar
            WorkspaceHairline()
            if let actionError = model.actionError {
                actionErrorBanner(actionError)
                WorkspaceHairline()
            }
            Group {
                if let loadError = model.loadError {
                    paletteMessage(
                        title: "Couldn't load snippets",
                        detail: loadError,
                        actionTitle: "Try again",
                        action: { model.refreshNow() }
                    )
                } else if model.rows.isEmpty {
                    emptyState
                } else {
                    HStack(spacing: 0) {
                        PaletteResultsList(model: model, controller: controller)
                            .frame(width: 340)
                        WorkspaceHairline().frame(width: 1)
                        if let row = model.selectedRow {
                            PalettePreview(model: model, snippet: row.snippet, groupPath: row.groupPath)
                        } else {
                            Spacer().frame(maxWidth: .infinity, maxHeight: .infinity)
                                .background(WorkspaceTheme.paper)
                        }
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            WorkspaceHairline()
            PaletteHintBar(hasSelection: model.selectedRow != nil)
        }
        .frame(width: SearchPaletteController.panelSize.width, height: SearchPaletteController.panelSize.height)
        .background(WorkspaceTheme.paper)
    }

    private func actionErrorBanner(_ message: String) -> some View {
        HStack(spacing: 8) {
            Text(message)
                .font(WorkspaceTheme.caption)
                .foregroundStyle(WorkspaceTheme.ink)
                .lineLimit(2)
            Spacer(minLength: 8)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(KzColor.warningSoft)
        .accessibilityElement(children: .combine)
    }

    private var searchBar: some View {
        HStack(spacing: 10) {
            PaletteSearchField(model: model)
            if !model.query.isEmpty {
                Button("Clear") { model.clearQuery() }
                    .buttonStyle(.borderless)
                    .controlSize(.small)
                    .font(WorkspaceTheme.caption)
                    .accessibilityLabel("Clear search")
            }
            WorkspaceKeyHint(keys: "esc", title: nil)
        }
        .padding(.horizontal, 16)
        .frame(height: 48)
    }

    private var emptyState: some View {
        paletteMessage(
            title: model.hasSnippets ? "No matching snippets" : "No snippets yet",
            detail: model.hasSnippets
                ? "Try fewer letters, or search by trigger or content."
                : "Create snippets in the Kudzu window.",
            actionTitle: nil,
            action: nil
        )
    }

    private func paletteMessage(
        title: String,
        detail: String,
        actionTitle: String?,
        action: (() -> Void)?
    ) -> some View {
        VStack(spacing: 8) {
            Spacer(minLength: 0)
            Text(title)
                .font(WorkspaceTheme.label)
                .foregroundStyle(WorkspaceTheme.ink)
            Text(detail)
                .font(WorkspaceTheme.body)
                .foregroundStyle(WorkspaceTheme.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 320)
            if let actionTitle, let action {
                Button(actionTitle, action: action)
                    .controlSize(.small)
            }
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(24)
    }
}

// MARK: - Search field

/// Search input backed by `NSTextField` so the panel can focus it immediately when opened.
private struct PaletteSearchField: View {
    let model: SearchPaletteModel
    @Environment(\.kzSnapshotRendering) private var isSnapshot

    var body: some View {
        if isSnapshot {
            Text(model.query.isEmpty ? "Search snippets" : model.query)
                .font(.system(size: 15, weight: .regular))
                .foregroundStyle(model.query.isEmpty ? WorkspaceTheme.secondary : WorkspaceTheme.ink)
                .lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            PaletteAppKitSearchField(model: model)
                .frame(maxWidth: .infinity)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

private struct PaletteAppKitSearchField: NSViewRepresentable {
    let model: SearchPaletteModel

    func makeNSView(context: Context) -> NSTextField {
        let field = FocusingTextField()
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        let font = NSFont.systemFont(ofSize: 15, weight: .regular)
        field.font = font
        field.textColor = NSColor.labelColor
        field.setAccessibilityPlaceholderValue("Search snippets")
        field.kzPlaceholder = NSAttributedString(
            string: "Search snippets",
            attributes: [
                .font: font,
                .foregroundColor: NSColor.secondaryLabelColor,
            ]
        )
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
        let isComposing = (field.currentEditor() as? NSTextInputClient)?.hasMarkedText() == true
        if !isComposing, field.stringValue != model.query {
            field.stringValue = model.query
            if let editor = field.currentEditor() {
                editor.selectedRange = NSRange(location: (model.query as NSString).length, length: 0)
            }
        }
        field.textColor = NSColor.labelColor
    }

    func makeCoordinator() -> Coordinator { Coordinator(model: model) }

    final class FocusingTextField: NSTextField {
        var kzPlaceholder: NSAttributedString? { didSet { needsDisplay = true } }

        override var stringValue: String { didSet { needsDisplay = true } }

        override func textDidChange(_ notification: Notification) {
            super.textDidChange(notification)
            needsDisplay = true
        }

        override func draw(_ dirtyRect: NSRect) {
            super.draw(dirtyRect)
            guard stringValue.isEmpty, let kzPlaceholder, let font else { return }
            let baseline = bounds.midY + font.capHeight / 2
            kzPlaceholder.draw(at: NSPoint(x: 2, y: baseline - font.ascender))
        }

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
    let controller: SearchPaletteController?

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(Array(model.rows.enumerated()), id: \.element.id) { index, row in
                        PaletteResultRow(row: row, isSelected: index == model.selectedIndex,
                                         preview: model.render(row.snippet, placeholders: true).text)
                            .id(row.id)
                            .contentShape(Rectangle())
                            .onTapGesture {
                                model.select(row.id)
                                if NSApp.currentEvent?.clickCount == 2 { controller?.insertSelection() }
                            }
                    }
                }
                .padding(.vertical, 4)
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
    let preview: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(alignment: .center, spacing: 8) {
                Text(Self.highlighted(row.snippet.name, row.nameHighlights))
                    .font(WorkspaceTheme.label)
                    .foregroundStyle(WorkspaceTheme.ink)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer(minLength: 8)
                if row.snippet.isFavorite {
                    Text("★")
                        .font(WorkspaceTheme.caption)
                        .foregroundStyle(WorkspaceTheme.secondary)
                        .accessibilityLabel("Favorite")
                }
                if let firstTrigger = row.snippet.triggers.first {
                    WorkspaceTriggerLabel(trigger: firstTrigger)
                }
                if row.snippet.triggers.count > 1 {
                    Text("+\(row.snippet.triggers.count - 1)")
                        .font(WorkspaceTheme.caption)
                        .foregroundStyle(WorkspaceTheme.secondary)
                }
            }
            HStack(spacing: 8) {
                Text(Self.excerpt(preview))
                    .font(WorkspaceTheme.caption)
                    .foregroundStyle(WorkspaceTheme.secondary)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer(minLength: 4)
                if let groupPath = row.groupPath {
                    Text(groupPath)
                        .font(WorkspaceTheme.caption)
                        .foregroundStyle(WorkspaceTheme.secondary)
                        .lineLimit(1)
                        .truncationMode(.head)
                }
            }
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(isSelected ? WorkspaceTheme.selection : Color.clear)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }

    static func highlighted(_ name: String, _ offsets: [Int]) -> AttributedString {
        guard !offsets.isEmpty else { return AttributedString(name) }
        let matched = Set(offsets)
        let matchFont = Font.system(size: 13, weight: .semibold)
        var result = AttributedString()
        var run = ""
        var runMatched = false
        func flush() {
            guard !run.isEmpty else { return }
            var part = AttributedString(run)
            if runMatched {
                part.foregroundColor = WorkspaceTheme.accent
                part.font = matchFont
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

    static func excerpt(_ content: String) -> String {
        content.prefix(160).split(whereSeparator: \.isWhitespace).joined(separator: " ")
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
                    .font(WorkspaceTheme.label)
                    .foregroundStyle(WorkspaceTheme.ink)
                    .lineLimit(2)
                HStack(spacing: 6) {
                    ForEach(Array(snippet.triggers.enumerated()), id: \.offset) { _, trigger in
                        WorkspaceTriggerLabel(trigger: trigger)
                    }
                    Spacer(minLength: 8)
                    if let groupPath {
                        Text(groupPath)
                            .font(WorkspaceTheme.caption)
                            .foregroundStyle(WorkspaceTheme.secondary)
                            .lineLimit(1)
                    }
                }
            }
            .padding(16)

            WorkspaceHairline()

            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    if result.text.isEmpty {
                        Text("This snippet is empty.")
                            .font(WorkspaceTheme.body)
                            .foregroundStyle(WorkspaceTheme.secondary)
                    } else {
                        Text(Self.attributedPreview(result))
                            .font(WorkspaceTheme.body)
                            .foregroundStyle(WorkspaceTheme.ink)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    if model.hasFormFields(snippet) {
                        Text("Form fields show their default values.")
                            .font(WorkspaceTheme.caption)
                            .foregroundStyle(WorkspaceTheme.secondary)
                    }
                }
                .padding(16)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(WorkspaceTheme.navigation)
    }

    static func attributedPreview(_ result: RenderResult) -> AttributedString {
        guard let offset = result.cursorOffset else { return AttributedString(result.text) }
        let utf16 = result.text.utf16
        let clamped = min(max(offset, 0), utf16.count)
        let index = String.Index(utf16Offset: clamped, in: result.text)
        var caret = AttributedString("\u{258F}")
        caret.foregroundColor = WorkspaceTheme.accent
        return AttributedString(String(result.text[..<index])) + caret + AttributedString(String(result.text[index...]))
    }
}

// MARK: - Hints

struct PaletteHintBar: View {
    let hasSelection: Bool

    var body: some View {
        HStack(spacing: 12) {
            Text("Kudzu")
                .font(WorkspaceTheme.caption)
                .foregroundStyle(WorkspaceTheme.accent)
            Spacer()
            if hasSelection {
                WorkspaceKeyHint(keys: "↵", title: "Insert")
                Text("·").font(WorkspaceTheme.caption).foregroundStyle(WorkspaceTheme.secondary).accessibilityHidden(true)
                WorkspaceKeyHint(keys: "⌘↵", title: "Copy")
                Text("·").font(WorkspaceTheme.caption).foregroundStyle(WorkspaceTheme.secondary).accessibilityHidden(true)
                WorkspaceKeyHint(keys: "⌘E", title: "Edit")
            } else {
                WorkspaceKeyHint(keys: "esc", title: "Close")
            }
        }
        .padding(.horizontal, 16)
        .frame(height: 32)
    }
}
