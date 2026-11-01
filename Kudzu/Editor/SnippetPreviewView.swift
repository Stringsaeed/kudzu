import KudzuCore
import SwiftUI

/// Live render of the template, with the caret position and any diagnostics.
struct SnippetPreviewView: View {
    @Environment(LibraryModel.self) private var library
    var content: String
    /// The snippet being edited, so that including itself is reported before it expands once.
    var rootSnippetID: UUID?

    var body: some View {
        let result = render()
        VStack(alignment: .leading, spacing: 8) {
            Text("Preview")
                .font(.headline)
            Group {
                if content.isEmpty {
                    Text("The expanded text appears here.")
                        .foregroundStyle(.tertiary)
                } else {
                    Text(Self.attributed(result))
                        .textSelection(.enabled)
                }
            }
            .font(.body)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(10)
            .background(.fill.quaternary, in: RoundedRectangle(cornerRadius: 8))

            if !result.diagnostics.isEmpty {
                DiagnosticsList(diagnostics: result.diagnostics)
            }
        }
    }

    private func render() -> RenderResult {
        let parsed = TemplateParser.parse(content)
        let info = TemplateAnalysis.analyze(parsed)
        // Form fields show their default, or a ‹name› placeholder, as the form would on first open.
        var formValues: [String: String] = [:]
        for field in info.formFields {
            formValues[field.name] = field.defaultValue ?? field.options.first ?? "‹\(field.name)›"
        }
        let contents = library.contentByID
        let context = RenderContext(
            now: .now,
            calendar: .current,
            timeZone: .current,
            locale: .current,
            clipboard: "‹clipboard›",
            formValues: formValues,
            resolveSnippet: { contents[$0] },
            rootSnippetID: rootSnippetID
        )
        return TemplateRenderer.render(content, context: context)
    }

    /// The rendered text with a thin accent-colored bar where the caret will land.
    static func attributed(_ result: RenderResult) -> AttributedString {
        guard let offset = result.cursorOffset else { return AttributedString(result.text) }
        let utf16 = result.text.utf16
        let clamped = min(max(offset, 0), utf16.count)
        let index = String.Index(utf16Offset: clamped, in: result.text)
        var caret = AttributedString("\u{258F}")
        caret.foregroundColor = .accentColor
        return AttributedString(String(result.text[..<index])) + caret + AttributedString(String(result.text[index...]))
    }
}

/// Template problems with severity icons.
struct DiagnosticsList: View {
    var diagnostics: [TemplateDiagnostic]
    private let limit = 6

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(Array(diagnostics.prefix(limit).enumerated()), id: \.offset) { _, diagnostic in
                Label {
                    Text(diagnostic.message)
                } icon: {
                    Image(systemName: diagnostic.severity == .error ? "xmark.octagon.fill" : "exclamationmark.triangle.fill")
                        .foregroundStyle(diagnostic.severity == .error ? Color.red : Color.orange)
                }
                .font(.callout)
            }
            if diagnostics.count > limit {
                Text("+\(diagnostics.count - limit) more")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .accessibilityElement(children: .contain)
    }
}
