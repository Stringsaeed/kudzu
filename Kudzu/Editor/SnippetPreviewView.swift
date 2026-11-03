import KudzuCore
import KudzuUI
import SwiftUI

/// Live render of the template, with the caret position and any diagnostics.
struct SnippetPreviewView: View {
    @Environment(LibraryModel.self) private var library
    var content: String
    /// The snippet being edited, so that including itself is reported before it expands once.
    var rootSnippetID: UUID?
    /// When false, only the preview well is shown (inside a disclosure).
    var showsLabel = true

    var body: some View {
        let result = Self.render(content, rootSnippetID: rootSnippetID, contents: library.contentByID)
        let well = VStack(alignment: .leading, spacing: KzSpace.s2) {
                Group {
                    if content.isEmpty {
                        Text("The expanded text appears here.")
                            .foregroundStyle(KzColor.inkMuted)
                    } else {
                        Text(Self.attributed(result))
                            .foregroundStyle(KzColor.ink)
                            .textSelection(.enabled)
                    }
                }
                .kzTextStyle(.content)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 14)
                .padding(.vertical, KzSpace.s3)
                .background(KzColor.surfaceSunken, in: RoundedRectangle(cornerRadius: KzRadius.lg))

                if !result.diagnostics.isEmpty {
                    DiagnosticsList(diagnostics: result.diagnostics)
                }
            }
        if showsLabel {
            KzField("Preview") { well }
        } else {
            well
        }
    }

    /// Renders a template as the preview shows it: form fields at their default (or a ‹name›
    /// placeholder when `placeholders` is true) and a placeholder or actual value for the clipboard.
    static func render(
        _ content: String,
        rootSnippetID: UUID?,
        contents: [UUID: String],
        now: Date = .now,
        placeholders: Bool = true,
        clipboard: () -> String? = { NSPasteboard.general.string(forType: .string) }
    ) -> RenderResult {
        let resolveSnippet: @Sendable (UUID) -> String? = { contents[$0] }
        let fields = TemplateAnalysis.formFields(in: content, rootSnippetID: rootSnippetID, resolveSnippet: resolveSnippet)
        var formValues: [String: String] = [:]
        for field in fields {
            formValues[field.name] = field.defaultValue ?? field.options.first ?? (placeholders ? "‹\(field.name)›" : "")
        }
        let clipboardValue: String?
        if placeholders {
            clipboardValue = "‹clipboard›"
        } else if TemplateAnalysis.usesClipboard(in: content, rootSnippetID: rootSnippetID, resolveSnippet: resolveSnippet) {
            clipboardValue = clipboard()
        } else {
            clipboardValue = nil
        }
        let context = RenderContext(
            now: now,
            calendar: .current,
            timeZone: .current,
            locale: .current,
            clipboard: clipboardValue,
            formValues: formValues,
            resolveSnippet: resolveSnippet,
            rootSnippetID: rootSnippetID
        )
        let rendered = TemplateRenderer.render(content, context: context)
        let formDiagnostics = TemplateAnalysis.formDiagnostics(
            in: content,
            rootSnippetID: rootSnippetID,
            resolveSnippet: resolveSnippet
        )
        guard !formDiagnostics.isEmpty else { return rendered }
        var combined = rendered.diagnostics
        for diagnostic in formDiagnostics where !combined.contains(where: {
            $0.message == diagnostic.message && $0.range == diagnostic.range
        }) {
            combined.append(diagnostic)
        }
        return RenderResult(text: rendered.text, cursorOffset: rendered.cursorOffset, diagnostics: combined)
    }

    /// The rendered text with a thin `vine-ink` bar where the caret will land.
    static func attributed(_ result: RenderResult) -> AttributedString {
        guard let offset = result.cursorOffset else { return AttributedString(result.text) }
        let utf16 = result.text.utf16
        let clamped = min(max(offset, 0), utf16.count)
        let index = String.Index(utf16Offset: clamped, in: result.text)
        var caret = AttributedString("\u{258F}")
        caret.foregroundColor = KzColor.vineInk
        return AttributedString(String(result.text[..<index])) + caret + AttributedString(String(result.text[index...]))
    }
}

/// The one-line plain preview a list row shows.
enum SnippetPreviewText {
    /// The expanded text on one line, or nil when the snippet is empty.
    @MainActor
    static func line(for snippet: SnippetRecord, contents: [UUID: String]) -> String? {
        guard !snippet.content.isEmpty else { return nil }
        let text = SnippetPreviewView.render(snippet.content, rootSnippetID: snippet.id, contents: contents).text
        // The row shows one line; collapsing early keeps long templates cheap to lay out.
        let line = text.prefix(240).split(whereSeparator: \.isNewline).joined(separator: " ")
        return line.isEmpty ? nil : line
    }
}

/// Template problems with severity icons.
struct DiagnosticsList: View {
    var diagnostics: [TemplateDiagnostic]
    private let limit = 6

    var body: some View {
        VStack(alignment: .leading, spacing: KzSpace.s1) {
            ForEach(Array(diagnostics.prefix(limit).enumerated()), id: \.offset) { _, diagnostic in
                EditorNote(
                    icon: diagnostic.severity == .error ? .error : .warning,
                    tone: diagnostic.severity == .error ? .danger : .warning,
                    text: AttributedString(diagnostic.message)
                )
            }
            if diagnostics.count > limit {
                Text("\(diagnostics.count - limit) more")
                    .kzTextStyle(.caption)
                    .foregroundStyle(KzColor.inkMuted)
            }
        }
        .accessibilityElement(children: .contain)
    }
}
