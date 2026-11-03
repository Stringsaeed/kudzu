import AppKit
import KudzuCore
import KudzuUI
import SwiftUI

/// Lets SwiftUI controls outside the text view (the window's Insert button and focus frame) act on it.
///
/// Create one, hand it to the editor with `.templateEditorController(_:)`, then:
/// - call `insert(_:)` or build the Insert button's menu with `insertMenu()`;
/// - read `isFocused` to draw `KzEditorFrame`'s focus edge.
@MainActor @Observable
final class TemplateEditorController {
    @ObservationIgnored fileprivate weak var coordinator: TemplateEditorCoordinator?
    /// Whether the text view is the first responder. Observable, so a surrounding `KzEditorFrame` updates.
    private(set) var isFocused = false

    /// Creates a controller to pass to `.templateEditorController(_:)`.
    init() {}

    /// Makes the content editor first responder. Returns false when the text view is not in a window yet.
    @discardableResult
    func focus() -> Bool {
        guard let textView = coordinator?.textView, textView.isEditable,
              let window = textView.window else { return false }
        return window.makeFirstResponder(textView)
    }

    /// Inserts an item's chip at the editor's selection and gives the editor focus.
    func insert(_ item: TemplateInsertItem) {
        coordinator?.insertFromMenu(item)
    }

    /// The Insert menu's elements: one item per tag kind, with its icon. Use inside a `KzButton(menu:)` or `KzMenu`.
    func insertMenu() -> [KzMenuElement] {
        TemplateInsertItem.allCases.map { item in
            .item(KzMenuItem(item.title, icon: ChipStyle.icon(for: item), id: item.id) { [weak self] in self?.insert(item) })
        }
    }

    fileprivate func setFocused(_ focused: Bool) {
        if isFocused != focused { isFocused = focused }
    }
}

extension EnvironmentValues {
    /// The controller the surrounding window uses to drive the editor, if it supplied one.
    @Entry var templateEditorController: TemplateEditorController?
}

extension View {
    /// Attaches a controller to a `TemplateTextEditor` inside this view, for the Insert button and the focus frame.
    func templateEditorController(_ controller: TemplateEditorController) -> some View {
        environment(\.templateEditorController, controller)
    }
}

/// Hosts the TextKit 1 chip text view in SwiftUI.
struct ChipTextViewRepresentable: NSViewRepresentable {
    @Environment(\.isEnabled) private var isEnabled
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
        textView.onFocusChange = { [weak controller] focused in controller?.setFocused(focused) }
        context.coordinator.update(configuration)
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        controller.coordinator = context.coordinator
        context.coordinator.update(configuration)
        if let textView = context.coordinator.textView {
            if !isEnabled, textView.window?.firstResponder === textView {
                textView.window?.makeFirstResponder(nil)
            }
            textView.isEditable = isEnabled
            textView.isSelectable = isEnabled
        }
    }

    static func dismantleNSView(_ scrollView: NSScrollView, coordinator: TemplateEditorCoordinator) {
        coordinator.teardown()
    }
}
