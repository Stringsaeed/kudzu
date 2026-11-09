// Preview harness: the editor in plain and formatted mode, light and dark, with every chip tone,
// an image, and the published source underneath so round trips can be checked by eye.

import KudzuCore
import KudzuDesign
import SwiftUI

/// A self-contained editor playground (also usable from the design gallery).
public struct TemplateEditorPreview: View {
    @State private var value: String
    @State private var formatting: Formatting?
    @State private var handle = TemplateEditorHandle()
    @State private var event = "Click a chip or type / to see callbacks."

    static let snippetID = "00000000-0000-0000-0000-000000000001"

    public static let plainSample = #"""
    Hi {{field name="name" default="there"}}, thanks for writing on {{date format="EEEE, MMMM d"}} at {{time}}.
    Pasted: {{clipboard trim=true}} · size {{choice name="size" options="S|M|L"}} · {{snippet id="\#(snippetID)"}}
    Broken: {{date format=}} · literal \{{not a tag}} · emoji 😀 e\#u{301}{{cursor}}
    """#

    static let tinyPNG = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAJUlEQVR4nGMICLj0n5aYYdSCUQtGLRi1YNSCUQtGLRi1YGhYAACpwH6M7RqhEQAAAABJRU5ErkJggg=="

    public static var formattedSample: (String, Formatting) {
        let source = "[Logo] Hello {{field name=\"who\"}}, see the docs.\nBest, {{snippet id=\"\(snippetID)\"}}"
        let spans = [
            FormatSpan(start: 0, end: 6, style: .image(dataUrl: tinyPNG, width: 48)),
            FormatSpan(start: 7, end: 12, style: .bold),
            FormatSpan(start: 13, end: 33, style: .italic),
            FormatSpan(start: 43, end: 47, style: .link(url: "https://kudzu.site/docs")),
            FormatSpan(start: 43, end: 47, style: .underline),
        ]
        return (source, Formatting(source: source, spans: spans))
    }

    public init(formatted: Bool = false) {
        if formatted {
            let (source, formatting) = Self.formattedSample
            _value = State(initialValue: source)
            _formatting = State(initialValue: formatting)
        } else {
            _value = State(initialValue: Self.plainSample)
            _formatting = State(initialValue: nil)
        }
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            TemplateEditor(
                value: $value,
                formatting: $formatting,
                allowFormatting: true,
                placeholder: "Type the text to expand. Press / to add a date, clipboard, fill-in field…",
                handle: handle,
                snippetName: { $0 == Self.snippetID ? "Signature" : nil },
                onChipClick: { tag, rect in event = "Chip \(tag.source) at \(Int(rect.minX)),\(Int(rect.minY))" },
                onSlashOpen: { position, _ in
                    event = "Slash at \(position)"
                    handle.replace(position, position + 1, "{{date}}")
                }
            )
            .frame(minHeight: 168)
            .background(Color.kz.surface, in: RoundedRectangle(cornerRadius: Radius.xl))
            .clipShape(RoundedRectangle(cornerRadius: Radius.xl))
            .depth(.card, in: RoundedRectangle(cornerRadius: Radius.xl))

            Text(event).font(.system(size: 12)).foregroundStyle(Color.kz.mutedForeground)
            Text(value)
                .font(Font.kz.mono(11))
                .foregroundStyle(Color.kz.mutedForeground)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(20)
        .frame(width: 640)
        .background(Color.kz.background)
    }
}

#Preview("Plain — light") {
    TemplateEditorPreview().preferredColorScheme(.light)
}

#Preview("Plain — dark") {
    TemplateEditorPreview().preferredColorScheme(.dark)
}

#Preview("Formatted — light") {
    TemplateEditorPreview(formatted: true).preferredColorScheme(.light)
}

#Preview("Formatted — dark") {
    TemplateEditorPreview(formatted: true).preferredColorScheme(.dark)
}

#Preview("Empty with placeholder") {
    @Previewable @State var value = ""
    TemplateEditor(value: $value, placeholder: "Type the text to expand. Press / to add a date, clipboard, fill-in field…")
        .frame(width: 520, height: 168)
}
