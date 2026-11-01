import KudzuCore
import SwiftUI

// Placeholder: owned by the chip-editor task. Keep this signature stable;
// the main window embeds it.

/// Rich editor for snippet template source that shows tags as inline chips.
struct TemplateTextEditor: View {
    /// Template source in docs/TEMPLATE_SYNTAX.md format. The editor reads and writes only this.
    @Binding var source: String
    /// Other snippets, for naming `{{snippet id=…}}` chips and the insert menu.
    var snippets: [SnippetRecord] = []

    var body: some View {
        TextEditor(text: $source)
            .font(.body.monospaced())
    }
}
