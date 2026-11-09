// Snippet content editor: the template source in a native NSTextView, with every tag shown as an
// atomic inline chip. The stored text never changes shape — chips are a presentation — so what
// you see round-trips byte for byte through `Template.parse` / `serialize`. Port of
// `components/app/editor/template-editor.tsx`.
//
// Give the editor a fresh identity per document (`.id(snippet.id)`) so each snippet starts with
// its own undo history; an external change to `value` replaces the content and clears history.

import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

public struct TemplateEditor: View {
    @Binding var value: String
    @Binding var formatting: Formatting?
    var allowFormatting: Bool
    var placeholder: String
    var handle: TemplateEditorHandle?
    var snippetName: (String) -> String?
    var onChipClick: (TagToken, CGRect) -> Void
    var onSlashOpen: (Int, CGRect) -> Void
    var onSelectionChange: (Range<Int>) -> Void
    var onEscape: (() -> Void)?
    var minHeight: CGFloat?

    @State private var ownHandle = TemplateEditorHandle()

    /// - Parameters:
    ///   - value: the stored template source.
    ///   - formatting: spans over `value`; `nil` is plain text.
    ///   - allowFormatting: shows the Plain text / Formatted selector, toolbar and help line.
    ///   - handle: imperative access (`insert`, `replace`, `coordsAt`, `focus`, formatting).
    ///   - snippetName: resolves nested snippet ids for `{{snippet}}` chip labels.
    ///   - onChipClick: a chip was clicked; the rect is the chip in screen coordinates.
    ///   - onSlashOpen: `/` typed at a word start; the source position of the `/` and the caret rect
    ///     (screen coordinates).
    ///   - minHeight: the text area grows with its content from this height and the view around
    ///     it scrolls (`min-h-[168px]`); `nil` fills the frame it is given and scrolls inside.
    public init(
        value: Binding<String>,
        formatting: Binding<Formatting?> = .constant(nil),
        allowFormatting: Bool = false,
        placeholder: String = "",
        handle: TemplateEditorHandle? = nil,
        snippetName: @escaping (String) -> String? = { _ in nil },
        onChipClick: @escaping (TagToken, CGRect) -> Void = { _, _ in },
        onSlashOpen: @escaping (Int, CGRect) -> Void = { _, _ in },
        onSelectionChange: @escaping (Range<Int>) -> Void = { _ in },
        onEscape: (() -> Void)? = nil,
        minHeight: CGFloat? = nil
    ) {
        _value = value
        _formatting = formatting
        self.allowFormatting = allowFormatting
        self.placeholder = placeholder
        self.handle = handle
        self.snippetName = snippetName
        self.onChipClick = onChipClick
        self.onSlashOpen = onSlashOpen
        self.onSelectionChange = onSelectionChange
        self.onEscape = onEscape
        self.minHeight = minHeight
    }

    public var body: some View {
        let handle = handle ?? ownHandle
        VStack(spacing: 0) {
            if allowFormatting {
                FormattingToolbar(handle: handle)
                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { handle.toolbarHeight = $0 }
            }
            TemplateTextView(
                value: $value, formatting: $formatting, placeholder: placeholder, handle: handle,
                snippetName: snippetName, onChipClick: onChipClick, onSlashOpen: onSlashOpen,
                onSelectionChange: onSelectionChange, onEscape: onEscape, minHeight: minHeight
            )
            if allowFormatting {
                Text(handle.isFormatted
                    ? "Select text to apply styles or add a link. Use Image to add a logo or picture."
                    : "Choose Formatted to add styles, links, and images.")
                    .font(.system(size: 12))
                    .foregroundStyle(Color.kz.mutedForeground)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .overlay(alignment: .top) { Rectangle().fill(Color.kz.hairline).frame(height: 1) }
            }
        }
    }
}

/// The NSTextView itself, without the toolbar and help line.
public struct TemplateTextView: NSViewRepresentable {
    @Binding var value: String
    @Binding var formatting: Formatting?
    var placeholder: String
    var handle: TemplateEditorHandle?
    var snippetName: (String) -> String?
    var onChipClick: (TagToken, CGRect) -> Void
    var onSlashOpen: (Int, CGRect) -> Void
    var onSelectionChange: (Range<Int>) -> Void
    var onEscape: (() -> Void)?
    /// Grow with the content from this height instead of scrolling inside (see `TemplateEditor`).
    var minHeight: CGFloat?

    public init(
        value: Binding<String>,
        formatting: Binding<Formatting?> = .constant(nil),
        placeholder: String = "",
        handle: TemplateEditorHandle? = nil,
        snippetName: @escaping (String) -> String? = { _ in nil },
        onChipClick: @escaping (TagToken, CGRect) -> Void = { _, _ in },
        onSlashOpen: @escaping (Int, CGRect) -> Void = { _, _ in },
        onSelectionChange: @escaping (Range<Int>) -> Void = { _ in },
        onEscape: (() -> Void)? = nil,
        minHeight: CGFloat? = nil
    ) {
        _value = value
        _formatting = formatting
        self.placeholder = placeholder
        self.handle = handle
        self.snippetName = snippetName
        self.onChipClick = onChipClick
        self.onSlashOpen = onSlashOpen
        self.onSelectionChange = onSelectionChange
        self.onEscape = onEscape
        self.minHeight = minHeight
    }

    public final class Coordinator {
        let controller: TemplateTextController
        var labels: [String] = []
        init(controller: TemplateTextController) { self.controller = controller }
    }

    public func makeCoordinator() -> Coordinator {
        Coordinator(controller: TemplateTextController(state: EditorState(source: value, formatting: formatting)))
    }

    public func makeNSView(context: Context) -> NSScrollView {
        let controller = context.coordinator.controller
        wire(controller)
        // The first layout ran before `snippetName` was set; label nested snippets now.
        controller.refreshChips()
        context.coordinator.labels = chipLabels(controller.state.source)
        handle?.attach(controller)
        return controller.scrollView
    }

    public func updateNSView(_ scrollView: NSScrollView, context: Context) {
        let controller = context.coordinator.controller
        wire(controller)
        controller.load(source: value, formatting: formatting)
        let labels = chipLabels(controller.state.source)
        if labels != context.coordinator.labels {
            context.coordinator.labels = labels
            controller.refreshChips()
        }
        if handle?.controller !== controller { handle?.attach(controller) }
        handle?.sync()
    }

    /// A growing editor is as tall as its content at the proposed width; otherwise it takes the
    /// frame it is offered (the scroll view has no size of its own).
    public func sizeThatFits(_ proposal: ProposedViewSize, nsView: NSScrollView, context: Context) -> CGSize? {
        let controller = context.coordinator.controller
        guard controller.growsFrom != nil else { return nil }
        let offered = proposal.width.flatMap { $0.isFinite && $0 > 0 ? $0 : nil }
        let width = offered ?? (nsView.bounds.width > 0 ? nsView.bounds.width : controller.textView.frame.width)
        return CGSize(width: width, height: controller.fittingHeight(width: width))
    }

    private func wire(_ controller: TemplateTextController) {
        let value = $value, formatting = $formatting, handle = handle
        controller.placeholder = placeholder
        controller.snippetName = snippetName
        controller.onChipClick = onChipClick
        controller.onSlashOpen = onSlashOpen
        controller.onSelectionChange = onSelectionChange
        controller.onEscape = onEscape
        controller.growsFrom = minHeight
        controller.onChange = { source, spans in
            if !value.wrappedValue.utf16.elementsEqual(source.utf16) { value.wrappedValue = source }
            let next = spans.map { Formatting(source: source, spans: $0) }
            let current = formatting.wrappedValue
            // Swift equality normalizes Unicode. Formatting.source is an exact UTF-16 contract:
            // even a canonically equivalent edit must update it or sourceSpans disables styling.
            if !FormattingHelpers.sameSpans(current?.spans, next?.spans) ||
                current.map({ !$0.source.utf16.elementsEqual(source.utf16) }) == true {
                formatting.wrappedValue = next
            }
        }
        controller.onStateChange = { handle?.sync() }
    }

    /// Chip labels as they would render now (detects snippet renames).
    private func chipLabels(_ source: String) -> [String] {
        Template.tokenize(source).compactMap { token in
            if case .tag(let tag) = token, tag.name == "snippet" { return Template.chipLabel(tag, snippetName: snippetName) }
            return nil
        }
    }
}
