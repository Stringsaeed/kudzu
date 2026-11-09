// Injection points for the screens other features own, so the library compiles without them:
// the snippet editor (T8), Insights / Settings / Onboarding (T11) and the clipboard view (T10).

import KudzuCore
import KudzuDesign
import SwiftUI

/// The views `LibraryRootView` hosts but doesn't own. Each defaults to a placeholder.
///
/// ```swift
/// LibraryRootView(slots: LibrarySlots(
///     editor: { AnyView(SnippetEditorView(snippet: $0)) },
///     settings: { AnyView(SettingsView(section: $0)) }))
/// ```
public struct LibrarySlots {
    /// The editor for the selected snippet. Recreated per snippet id (`key={snippet.id}`).
    public var editor: @MainActor (Snippet) -> AnyView
    public var insights: @MainActor () -> AnyView
    /// The clipboard history for a filter (`nil` = History).
    public var clipboard: @MainActor (ClipFilter?) -> AnyView
    public var settings: @MainActor (SettingsSection?) -> AnyView
    /// Shown instead of the library until `settings.onboardingCompleted`.
    public var onboarding: @MainActor () -> AnyView
    /// The editor feature's app picker, injected by the host for group rules.
    public var groupAppPicker: @MainActor (Binding<[String]>, String) -> AnyView

    public init(
        editor: (@MainActor (Snippet) -> AnyView)? = nil,
        insights: (@MainActor () -> AnyView)? = nil,
        clipboard: (@MainActor (ClipFilter?) -> AnyView)? = nil,
        settings: (@MainActor (SettingsSection?) -> AnyView)? = nil,
        onboarding: (@MainActor () -> AnyView)? = nil,
        groupAppPicker: (@MainActor (Binding<[String]>, String) -> AnyView)? = nil
    ) {
        self.editor = editor ?? { AnyView(SlotPlaceholder(title: $0.name.isEmpty ? "Untitled" : $0.name, detail: "Snippet editor")) }
        self.insights = insights ?? { AnyView(SlotPlaceholder(title: "Insights", detail: "Usage tiles and ranked snippets")) }
        self.clipboard = clipboard ?? { AnyView(SlotPlaceholder(title: "Clipboard", detail: $0?.rawValue ?? "History")) }
        self.settings = settings ?? { AnyView(SlotPlaceholder(title: "Settings", detail: LibraryBreadcrumbs.settingsLabel($0 ?? .general))) }
        self.groupAppPicker = groupAppPicker ?? { _, _ in
            AnyView(SlotPlaceholder(title: "Apps", detail: "App selection is unavailable."))
        }
        self.onboarding = onboarding ?? { AnyView(SlotPlaceholder(title: "Welcome to Kudzu", detail: "Onboarding")) }
    }
}

/// What an unfilled slot shows.
struct SlotPlaceholder: View {
    var title: String
    var detail: String

    var body: some View {
        VStack(spacing: 4) {
            Text(title).font(.system(size: 14, weight: .medium))
            Text(detail).font(.system(size: 12)).mutedForeground()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
