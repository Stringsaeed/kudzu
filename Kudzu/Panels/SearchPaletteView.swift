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
            KzColor.line.frame(height: 1)
            if model.rows.isEmpty {
                emptyState
            } else {
                HStack(spacing: 0) {
                    PaletteResultsList(model: model, controller: controller)
                        .frame(width: 340)
                    KzColor.line.frame(width: 1)
                    if let row = model.selectedRow {
                        PalettePreview(model: model, snippet: row.snippet, groupPath: row.groupPath)
                    } else {
                        Spacer().frame(maxWidth: .infinity, maxHeight: .infinity)
                            .background(KzColor.surfaceRaised)
                    }
                }
            }
            KzColor.line.frame(height: 1)
            PaletteHintBar()
        }
        .frame(width: SearchPaletteController.panelSize.width, height: SearchPaletteController.panelSize.height)
    }

    private var searchBar: some View {
        HStack(spacing: KzSpace.s3) {
            KzIcon(.search, size: 16)
                .foregroundStyle(KzColor.inkMuted)
            PaletteSearchField(model: model)
            if !model.query.isEmpty {
                Button {
                    model.query = ""
                } label: {
                    KzIcon(.x, size: 12)
                        .foregroundStyle(KzColor.inkMuted)
                        .frame(width: 18, height: 18)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Clear search")
            }
            KzKbd("esc")
        }
        .padding(.horizontal, KzSpace.s4)
        .frame(height: 54)
    }

    private var emptyState: some View {
        KzEmptyState(
            model.hasSnippets ? "No matching snippets" : "No snippets yet",
            icon: model.hasSnippets ? .search : .leaf,
            message: model.hasSnippets
                ? "Try fewer letters, or search by trigger or content."
                : "Create snippets in the Kudzu window."
        )
    }
}

// MARK: - Search field

/// Large search input styled in the `KzSearchField` look, backed by `NSTextField` so the panel
/// can focus it immediately when opened, with a SwiftUI fallback for offscreen snapshots.
private struct PaletteSearchField: View {
    let model: SearchPaletteModel
    @Environment(\.kzSnapshotRendering) private var isSnapshot

    var body: some View {
        if isSnapshot {
            Text(model.query.isEmpty ? "Search snippets" : model.query)
                .kzTextStyle(.titleLg, matchLineBox: false)
                .foregroundStyle(model.query.isEmpty ? KzColor.inkMuted : KzColor.ink)
                .lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            PaletteAppKitSearchField(model: model)
                .frame(maxWidth: .infinity, minHeight: 28, maxHeight: 28)
        }
    }
}

private struct PaletteAppKitSearchField: NSViewRepresentable {
    let model: SearchPaletteModel

    func makeNSView(context: Context) -> NSTextField {
        KzFonts.register()
        let field = FocusingTextField()
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        let font = NSFont(name: KzFonts.postScriptName(family: .sans, weight: 400), size: 18)
            ?? .systemFont(ofSize: 18, weight: .regular)
        field.font = font
        field.textColor = KzColorToken.ink.nsColor
        field.placeholderAttributedString = NSAttributedString(
            string: "Search snippets",
            attributes: [
                .font: font,
                .foregroundColor: KzColorToken.inkMuted.nsColor,
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
        if field.stringValue != model.query { field.stringValue = model.query }
        field.textColor = KzColorToken.ink.nsColor
    }

    func makeCoordinator() -> Coordinator { Coordinator(model: model) }

    /// Takes focus as soon as it lands in the panel, which the first `show()` can't do because
    /// SwiftUI builds the view after the panel is ordered in.
    final class FocusingTextField: NSTextField {
        override class var cellClass: AnyClass? {
            get { CenteredTextFieldCell.self }
            set {}
        }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            window?.makeFirstResponder(self)
        }
    }

    /// Centres the text, placeholder and caret vertically. `NSTextFieldCell` pins one line to the top
    /// of its frame, and Geist's tall line box made the text sit visibly below the search icon.
    final class CenteredTextFieldCell: NSTextFieldCell {
        private func centered(_ rect: NSRect) -> NSRect {
            guard let font else { return rect }
            let lineHeight = ceil(NSLayoutManager().defaultLineHeight(for: font))
            let height = min(rect.height, lineHeight)
            // Optical centre: put the middle of the capitals (baseline − cap height / 2) on the frame's
            // middle, which lines the text up with the search icon beside it. The field is flipped.
            let opticalTop = font.ascender - font.capHeight / 2
            let top = min(max(rect.minY, rect.midY - opticalTop), rect.maxY - height)
            return NSRect(x: rect.minX, y: round(top), width: rect.width, height: height)
        }

        override func titleRect(forBounds rect: NSRect) -> NSRect { centered(super.titleRect(forBounds: rect)) }

        override func drawingRect(forBounds rect: NSRect) -> NSRect { centered(super.drawingRect(forBounds: rect)) }

        override func edit(withFrame rect: NSRect, in controlView: NSView, editor textObj: NSText,
                           delegate: Any?, event: NSEvent?) {
            super.edit(withFrame: centered(rect), in: controlView, editor: textObj, delegate: delegate, event: event)
        }

        override func select(withFrame rect: NSRect, in controlView: NSView, editor textObj: NSText,
                             delegate: Any?, start selStart: Int, length selLength: Int) {
            super.select(withFrame: centered(rect), in: controlView, editor: textObj, delegate: delegate,
                         start: selStart, length: selLength)
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
                LazyVStack(spacing: 2) {
                    ForEach(Array(model.rows.enumerated()), id: \.element.id) { index, row in
                        PaletteResultRow(row: row, isSelected: index == model.selectedIndex)
                            .id(row.id)
                            .contentShape(Rectangle())
                            .onTapGesture {
                                // One gesture, so a single click selects without waiting out the
                                // double-click interval; the second click of a double-click inserts.
                                model.select(row.id)
                                if NSApp.currentEvent?.clickCount == 2 { controller?.insertSelection() }
                            }
                    }
                }
                .padding(KzSpace.s2)
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
    @State private var isHovered = false

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.md, style: .continuous)
        VStack(alignment: .leading, spacing: KzSpace.s1) {
            HStack(alignment: .center, spacing: KzSpace.s2) {
                Text(Self.highlighted(row.snippet.name, row.nameHighlights))
                    .kzTextStyle(.bodyStrong)
                    .foregroundStyle(KzColor.ink)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer(minLength: KzSpace.s2)
                if row.snippet.isFavorite {
                    KzIcon(.starFill, size: 12, label: "Favorite")
                        .foregroundStyle(KzColor.bloom)
                }
                if let firstTrigger = row.snippet.triggers.first {
                    KzTriggerChip(firstTrigger)
                }
                if row.snippet.triggers.count > 1 {
                    Text("+\(row.snippet.triggers.count - 1)")
                        .kzTextStyle(.caption)
                        .foregroundStyle(KzColor.inkMuted)
                }
            }
            HStack(spacing: KzSpace.s2) {
                Text(Self.excerpt(row.snippet.content))
                    .kzTextStyle(.callout)
                    .foregroundStyle(KzColor.inkMuted)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer(minLength: KzSpace.s1)
                if let groupPath = row.groupPath {
                    Text(groupPath)
                        .kzTextStyle(.caption)
                        .foregroundStyle(KzColor.inkMuted)
                        .lineLimit(1)
                        .truncationMode(.head)
                }
            }
        }
        .padding(.vertical, 10)
        .padding(.horizontal, KzSpace.s3)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background {
            if isSelected {
                shape.fill(KzColor.vineSoft)
            }
        }
        .kzStateLayer(shape, isHovered: !isSelected && isHovered, isPressed: false)
        .kzHover($isHovered)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }

    /// The name with matched characters highlighted in `vine-ink` and semibold Geist.
    static func highlighted(_ name: String, _ offsets: [Int]) -> AttributedString {
        guard !offsets.isEmpty else { return AttributedString(name) }
        KzFonts.register()
        let matched = Set(offsets)
        let matchFont: Font = {
            let psName = KzFonts.postScriptName(family: .sans, weight: 600)
            if KzFonts.isAvailable(psName) {
                return .custom(psName, fixedSize: 13)
            }
            return .system(size: 13, weight: .semibold)
        }()
        var result = AttributedString()
        var run = ""
        var runMatched = false
        func flush() {
            guard !run.isEmpty else { return }
            var part = AttributedString(run)
            if runMatched {
                part.foregroundColor = KzColor.vineInk
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

    /// The first line's worth of raw content, whitespace collapsed.
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
            VStack(alignment: .leading, spacing: KzSpace.s2) {
                Text(snippet.name)
                    .kzTextStyle(.title)
                    .foregroundStyle(KzColor.ink)
                    .lineLimit(2)
                HStack(spacing: KzSpace.s1) {
                    ForEach(Array(snippet.triggers.enumerated()), id: \.offset) { _, trigger in
                        KzTriggerChip(trigger)
                    }
                    Spacer(minLength: KzSpace.s2)
                    if let groupPath {
                        HStack(spacing: KzSpace.s1) {
                            KzIcon(.folder, size: 12)
                                .foregroundStyle(KzColor.inkMuted)
                            Text(groupPath)
                                .kzTextStyle(.caption)
                                .foregroundStyle(KzColor.inkMuted)
                                .lineLimit(1)
                        }
                    }
                }
            }
            .padding(KzSpace.s4)

            KzColor.line.frame(height: 1)

            ScrollView {
                VStack(alignment: .leading, spacing: KzSpace.s3) {
                    if result.text.isEmpty {
                        Text("This snippet is empty.")
                            .kzTextStyle(.body)
                            .foregroundStyle(KzColor.inkMuted)
                    } else {
                        Text(Self.attributedPreview(result))
                            .kzTextStyle(.content)
                            .foregroundStyle(KzColor.ink)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    if model.hasFormFields(snippet) {
                        HStack(spacing: KzSpace.s1) {
                            KzIcon(.list, size: 12)
                                .foregroundStyle(KzColor.inkMuted)
                            Text("Form fields show their default values.")
                                .kzTextStyle(.caption)
                                .foregroundStyle(KzColor.inkMuted)
                        }
                    }
                }
                .padding(KzSpace.s4)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(KzColor.surfaceRaised)
    }

    /// Rendered text with a `vine-ink` bar where the caret will land.
    static func attributedPreview(_ result: RenderResult) -> AttributedString {
        guard let offset = result.cursorOffset else { return AttributedString(result.text) }
        let utf16 = result.text.utf16
        let clamped = min(max(offset, 0), utf16.count)
        let index = String.Index(utf16Offset: clamped, in: result.text)
        var caret = AttributedString("\u{258F}")
        caret.foregroundColor = KzColor.vineInk
        return AttributedString(String(result.text[..<index])) + caret + AttributedString(String(result.text[index...]))
    }
}

// MARK: - Hints

private struct PaletteHintBar: View {
    var body: some View {
        HStack(spacing: KzSpace.s3) {
            KzIcon(.leaf, size: 14)
                .foregroundStyle(KzColor.inkMuted)
            Spacer()
            Hint(keys: "↵", title: "Insert")
            Text("·")
                .kzTextStyle(.caption)
                .foregroundStyle(KzColor.inkMuted)
                .accessibilityHidden(true)
            Hint(keys: "⌘↵", title: "Copy")
            Text("·")
                .kzTextStyle(.caption)
                .foregroundStyle(KzColor.inkMuted)
                .accessibilityHidden(true)
            Hint(keys: "⌘E", title: "Edit")
        }
        .padding(.horizontal, KzSpace.s4)
        .frame(height: 36)
    }

    private struct Hint: View {
        let keys: String
        let title: String

        var body: some View {
            HStack(spacing: KzSpace.s1) {
                KzKbd(keys)
                Text(title)
                    .kzTextStyle(.caption)
                    .foregroundStyle(KzColor.inkMuted)
            }
            .accessibilityElement(children: .combine)
        }
    }
}
