import SwiftUI

extension EnvironmentValues {
    /// True inside a `KzToolbarGroup`, where icon buttons are 28pt tall instead of 30 (`.kz-toolbar .kz-iconbtn`).
    @Entry public var kzInToolbarGroup: Bool = false
}

/// A small glass capsule that floats icon buttons over the detail pane: the one place glass appears
/// inside the content area. Keep it to 2–4 actions about the selected item; separate clusters
/// with `KzToolbarSeparator`. Never put it inside a menu or on the sidebar (no glass on glass).
///
/// ```swift
/// KzToolbarGroup {
///     KzIconButton(.star, label: "Favorite") { … }
///     KzToolbarSeparator()
///     KzIconButton(.more, label: "More") { … }
/// }
/// ```
public struct KzToolbarGroup<Content: View>: View {
    private let label: String?
    private let content: Content

    /// Creates a group; `label` names it for VoiceOver.
    public init(label: String? = nil, @ViewBuilder content: () -> Content) {
        self.label = label
        self.content = content()
    }

    public var body: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.lg)
        HStack(spacing: 2) {
            content
        }
        .environment(\.kzInToolbarGroup, true)
        .padding(3)
        .kzGlass(.strong, in: shape, blending: .withinWindow, shadow: .glass)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(label ?? "Toolbar")
    }
}

/// The 1×16pt `line` between clusters of buttons in a `KzToolbarGroup`.
public struct KzToolbarSeparator: View {
    /// Creates a separator.
    public init() {}

    public var body: some View {
        Rectangle()
            .fill(KzColor.line)
            .frame(width: 1, height: 16)
            .padding(.horizontal, 3)
            .accessibilityHidden(true)
    }
}
