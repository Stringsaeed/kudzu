import SwiftUI

/// The library window's content: ``LibraryRootView`` with every feature screen in its slots.
/// The shell hosts it with the shared ``AppModel`` in the environment (`@Environment(AppModel.self)`).
struct RootView: View {
    var body: some View {
        LibraryRootView(slots: .app)
    }
}

extension LibrarySlots {
    /// The real screens: the snippet editor (T8), Insights / Settings / Onboarding (T11), the
    /// clipboard history (T10) and the editor's app picker for group rules.
    @MainActor static var app: LibrarySlots {
        LibrarySlots(
            editor: { _ in AnyView(EditorSlot()) },
            insights: { AnyView(InsightsScreen()) },
            clipboard: { AnyView(ClipboardHistoryView(filter: $0)) },
            settings: { AnyView(SettingsScreen(section: $0)) },
            onboarding: { AnyView(OnboardingScreen()) },
            groupAppPicker: { value, label in AnyView(AppPicker(value: value, addLabel: label)) }
        )
    }
}

/// The editor pane, deleting through the library's action so selection, list focus and the
/// Undo toast behave as they do from the list.
private struct EditorSlot: View {
    @Environment(LibraryActions.self) private var actions

    var body: some View {
        SnippetEditorPane(onRequestDelete: { actions.requestDelete($0) })
    }
}
