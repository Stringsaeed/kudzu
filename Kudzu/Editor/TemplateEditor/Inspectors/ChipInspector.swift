import KudzuCore
import SwiftUI

/// Editor state a chip inspector needs beyond the tag itself.
struct ChipInspectorContext {
    var snippets: [SnippetRecord] = []
    var currentSnippetID: UUID?
    /// Whether a form name is already used by a control of a different kind elsewhere in the template.
    var nameConflicts: (String, FormFieldSpec.Kind) -> Bool = { _, _ in false }
}

/// Routes a chip to the inspector for its tag kind. Each inspector reports every valid change through `onChange`.
struct ChipInspector: View {
    let tag: KudzuCore.Tag
    let context: ChipInspectorContext
    let onChange: (KudzuCore.Tag) -> Void

    var body: some View {
        content
            .padding(14)
            .frame(width: 300)
    }

    @ViewBuilder private var content: some View {
        if let verbatim = tag.verbatim {
            UnrecognizedChipInspector(tag: tag, source: verbatim)
        } else {
            recognizedContent
        }
    }

    @ViewBuilder private var recognizedContent: some View {
        switch tag.name {
        case "date", "time": DateChipInspector(tag: tag, onChange: onChange)
        case "clipboard": ClipboardChipInspector(tag: tag, onChange: onChange)
        case "field": FieldChipInspector(tag: tag, context: context, onChange: onChange)
        case "choice": ChoiceChipInspector(tag: tag, context: context, onChange: onChange)
        case "snippet": SnippetChipInspector(tag: tag, context: context, onChange: onChange)
        default: CursorChipInspector(tag: tag)
        }
    }
}

/// Symbol, title and one-line explanation at the top of every inspector.
struct InspectorHeader: View {
    let tag: KudzuCore.Tag
    let title: String
    let subtitle: String

    var body: some View {
        let label = ChipLabel(tag: tag)
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: label.symbolName)
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(Color(nsColor: ChipStyle.tint(for: label.kind)))
                .frame(width: 28, height: 28)
                .background(RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(Color(nsColor: ChipStyle.tint(for: label.kind)).opacity(0.15)))
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.headline)
                Text(subtitle).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// A small red explanation under an invalid control.
struct ValidationMessage: View {
    let text: String?

    var body: some View {
        if let text {
            Label(text, systemImage: "exclamationmark.triangle.fill")
                .font(.caption)
                .foregroundStyle(.red)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

/// An unknown or malformed tag has no settings; the inspector shows what will be saved.
struct UnrecognizedChipInspector: View {
    let tag: KudzuCore.Tag
    let source: String

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            InspectorHeader(tag: tag, title: "Unrecognized tag",
                            subtitle: "Kudzu doesn't understand this tag, so it is left exactly as written and inserted as plain text. "
                                + "Delete the chip to remove it.")
            Text(source)
                .font(.system(.callout, design: .monospaced))
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(8)
                .background(.fill.quaternary, in: RoundedRectangle(cornerRadius: 6))
            ForEach(Array(TemplateParser.parse(source).diagnostics.prefix(1).enumerated()), id: \.offset) { _, diagnostic in
                ValidationMessage(text: diagnostic.message)
            }
        }
    }
}

/// The cursor chip has no settings, only an explanation.
struct CursorChipInspector: View {
    let tag: KudzuCore.Tag

    var body: some View {
        InspectorHeader(tag: tag, title: "Cursor",
                        subtitle: "After expanding, the caret lands here. Only the first cursor in a snippet counts.")
    }
}

/// Clipboard chip: a single trim option.
struct ClipboardChipInspector: View {
    let tag: KudzuCore.Tag
    let onChange: (KudzuCore.Tag) -> Void
    @State private var trim: Bool

    init(tag: KudzuCore.Tag, onChange: @escaping (KudzuCore.Tag) -> Void) {
        self.tag = tag
        self.onChange = onChange
        _trim = State(initialValue: tag.boolAttribute("trim"))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            InspectorHeader(tag: tag, title: "Clipboard", subtitle: "Inserts the clipboard's plain text.")
            Toggle("Trim whitespace and newlines", isOn: $trim)
        }
        .onChange(of: trim) { onChange(tag.setting("trim", flag: trim)) }
    }
}
