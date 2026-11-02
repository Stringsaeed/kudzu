import KudzuCore
import KudzuUI
import SwiftUI

/// Editor state a chip inspector needs beyond the tag itself.
struct ChipInspectorContext {
    var snippets: [SnippetRecord] = []
    var currentSnippetID: UUID?
    /// Whether a form name is already used by a control of a different kind elsewhere in the template.
    var nameConflicts: (String, FormFieldSpec.Kind) -> Bool = { _, _ in false }
}

/// Routes a chip to the inspector for its tag kind. Each inspector reports every valid change through `onChange`.
///
/// The popover chrome (glass, padding) comes from the panel that hosts this view; the content is `KzField`s and the
/// other Kudzu controls, `space-4` apart.
struct ChipInspector: View {
    /// The inspector's content width.
    static let width: CGFloat = 288

    let tag: KudzuCore.Tag
    let context: ChipInspectorContext
    let onChange: (KudzuCore.Tag) -> Void

    var body: some View {
        content
            .frame(width: Self.width, alignment: .leading)
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

/// Icon, title and one-line explanation at the top of every inspector.
struct InspectorHeader: View {
    let tag: KudzuCore.Tag
    let title: String
    let subtitle: String

    var body: some View {
        let label = ChipLabel(tag: tag)
        let isError = label.kind == .unknown
        let shape = RoundedRectangle(cornerRadius: KzRadius.md)
        HStack(alignment: .top, spacing: KzSpace.s3) {
            // The tag's own colours: `vine-ink` on `vine-soft`, or `danger` on `danger-soft` for an unrecognized tag.
            KzIcon(ChipStyle.icon(for: label.kind), size: 16)
                .foregroundStyle(isError ? KzColor.danger : KzColor.vineInk)
                .frame(width: 32, height: 32)
                .background(isError ? KzColor.dangerSoft : KzColor.vineSoft, in: shape)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .kzTextStyle(.headline)
                    .foregroundStyle(KzColor.ink)
                    .accessibilityAddTraits(.isHeader)
                Text(subtitle)
                    .kzTextStyle(.callout)
                    .foregroundStyle(KzColor.inkMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// A short explanation under an invalid control: `danger` with a warning icon, so colour is never the only signal.
struct ValidationMessage: View {
    let text: String?

    var body: some View {
        if let text {
            HStack(alignment: .firstTextBaseline, spacing: KzSpace.s2) {
                KzIcon(.warning, size: 12)
                    .accessibilityHidden(true)
                Text(text)
                    .kzTextStyle(.callout, matchLineBox: false)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .foregroundStyle(KzColor.danger)
            .accessibilityElement(children: .combine)
        }
    }
}

/// A labelled switch row, as in settings: the label on the left, a `KzToggle` on the right.
struct InspectorToggleRow: View {
    let title: String
    @Binding var isOn: Bool

    var body: some View {
        HStack(spacing: KzSpace.s3) {
            Text(title)
                .kzTextStyle(.body)
                .foregroundStyle(KzColor.ink)
            Spacer(minLength: 0)
            KzToggle(title, isOn: $isOn)
        }
    }
}

/// The inspectors' vertical rhythm.
struct InspectorStack<Content: View>: View {
    let content: Content

    init(@ViewBuilder content: () -> Content) { self.content = content() }

    var body: some View {
        VStack(alignment: .leading, spacing: KzSpace.s4) { content }
    }
}

/// An unknown or malformed tag has no settings; the inspector shows what will be saved.
struct UnrecognizedChipInspector: View {
    let tag: KudzuCore.Tag
    let source: String

    var body: some View {
        InspectorStack {
            InspectorHeader(tag: tag, title: "Unrecognized tag",
                            subtitle: "Kudzu doesn't understand this tag, so it is left exactly as written and inserted as plain text. "
                                + "Delete the chip to remove it.")
            Text(source)
                .kzTextStyle(.code, matchLineBox: false)
                .foregroundStyle(KzColor.danger)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(KzSpace.s2)
                .background(KzColor.dangerSoft, in: RoundedRectangle(cornerRadius: KzRadius.md))
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
        InspectorStack {
            InspectorHeader(tag: tag, title: "Clipboard", subtitle: "Inserts the clipboard's plain text.")
            InspectorToggleRow(title: "Trim whitespace and newlines", isOn: $trim)
        }
        .onChange(of: trim) { onChange(tag.setting("trim", flag: trim)) }
    }
}
