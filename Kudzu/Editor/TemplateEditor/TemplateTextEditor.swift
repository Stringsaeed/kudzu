import KudzuCore
import KudzuUI
import SwiftUI

/// Rich editor for snippet template source that shows tags as inline chips.
///
/// Type `/` at the start of a line or after a space to insert a tag. Click a chip, or select it and press
/// Return, to edit it.
///
/// This is the text area only: the surrounding window supplies the `KzEditorFrame`, the Insert button and the
/// hint bar. To drive them, create a `TemplateEditorController` and attach it with `.templateEditorController(_:)`.
struct TemplateTextEditor: View {
    /// Template source in docs/TEMPLATE_SYNTAX.md format. The editor reads and writes only this.
    @Binding var source: String
    /// Other snippets, for naming `{{snippet id=…}}` chips and the insert menu.
    var snippets: [SnippetRecord] = []
    /// The snippet being edited, so the snippet picker can exclude it and self-references are flagged.
    var currentSnippetID: UUID?
    /// Called with editor diagnostics (ranges in canonical source) whenever they change.
    var onDiagnostics: ([TemplateDiagnostic]) -> Void = { _ in }

    @State private var ownController = TemplateEditorController()
    @Environment(\.templateEditorController) private var injectedController

    /// Creates an editor bound to template source.
    init(source: Binding<String>, snippets: [SnippetRecord] = [], currentSnippetID: UUID? = nil,
         onDiagnostics: @escaping ([TemplateDiagnostic]) -> Void = { _ in }) {
        _source = source
        self.snippets = snippets
        self.currentSnippetID = currentSnippetID
        self.onDiagnostics = onDiagnostics
    }

    var body: some View {
        ChipTextViewRepresentable(source: $source, snippets: snippets, currentSnippetID: currentSnippetID,
                                  controller: injectedController ?? ownController, onDiagnostics: onDiagnostics)
    }
}

#Preview("Every chip kind") {
    @Previewable @State var source = """
        Hi {{field name="name" default="there"}},

        Thanks for reaching out on {{date format="EEEE"}}. I'll follow up by {{date offset="+3d"}} at {{time}}.
        Tone: {{choice name="tone" options="Friendly|Formal|Brief" default="Friendly"}}
        Pasted: {{clipboard trim=true}}
        {{snippet id="5F0B8E4A-3C1D-4C6B-9E8F-2A7D6B1C0E93"}} and {{snippet id="00000000-0000-0000-0000-000000000001"}}
        Broken: {{DATE}} and {{unknown tag}}
        {{cursor}}
        """
    @Previewable @State var controller = TemplateEditorController()
    let signature = SnippetRecord(id: UUID(uuidString: "5F0B8E4A-3C1D-4C6B-9E8F-2A7D6B1C0E93")!, name: "Signature",
                                  triggers: [";sig"], content: "— Ada")
    VStack(alignment: .leading, spacing: 8) {
        KzEditorFrame(isFocused: controller.isFocused) {
            KzButton("Insert", variant: .ghost, size: .sm, icon: .plus, iconEnd: .chevronDown, menuTitle: "Insert tag") {
                controller.insertMenu()
            }
        } content: {
            TemplateTextEditor(source: $source, snippets: [signature])
                .templateEditorController(controller)
        }
        .frame(height: 260)
        Text(source)
            .kzTextStyle(.code, matchLineBox: false)
            .foregroundStyle(KzColor.inkMuted)
            .textSelection(.enabled)
    }
    .padding()
    .frame(width: 560)
}
