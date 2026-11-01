import AppKit
import KudzuCore
import SwiftUI

/// Lets SwiftUI controls outside the text view (the "+" menu) act on it.
@MainActor
final class TemplateEditorController {
    fileprivate weak var coordinator: TemplateEditorCoordinator?

    /// Inserts an item's chip at the editor's selection.
    func insert(_ item: TemplateInsertItem) {
        coordinator?.insertFromMenu(item)
    }
}

/// Hosts the TextKit 1 chip text view in SwiftUI.
struct ChipTextViewRepresentable: NSViewRepresentable {
    @Binding var source: String
    var snippets: [SnippetRecord]
    var currentSnippetID: UUID?
    var controller: TemplateEditorController
    var onDiagnostics: ([TemplateDiagnostic]) -> Void

    private var configuration: TemplateEditorConfiguration {
        TemplateEditorConfiguration(source: $source, snippets: snippets, currentSnippetID: currentSnippetID,
                                    onDiagnostics: onDiagnostics)
    }

    func makeCoordinator() -> TemplateEditorCoordinator {
        TemplateEditorCoordinator(configuration: configuration)
    }

    func makeNSView(context: Context) -> NSScrollView {
        let (scrollView, textView) = ChipTextView.makeScrollView()
        context.coordinator.attach(textView)
        controller.coordinator = context.coordinator
        context.coordinator.update(configuration)
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        controller.coordinator = context.coordinator
        context.coordinator.update(configuration)
    }

    static func dismantleNSView(_ scrollView: NSScrollView, coordinator: TemplateEditorCoordinator) {
        coordinator.teardown()
    }
}
