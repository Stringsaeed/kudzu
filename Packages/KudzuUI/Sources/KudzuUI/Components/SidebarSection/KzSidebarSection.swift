import SwiftUI

/// A titled block of sidebar items ("Groups"), with an optional action at the right of its heading
/// (the + for a new group).
///
/// The heading is `caption` in `ink-muted`, sentence case, never uppercase. Stack sections `space-4`
/// apart; `KzAppShell` does this for you.
public struct KzSidebarSection<Action: View, Content: View>: View {
    private let title: String?
    private let action: Action
    private let content: Content

    /// Creates a section.
    /// - Parameters:
    ///   - title: The heading; `nil` for an untitled block such as the library items.
    ///   - action: A small `KzIconButton` shown at the right of the heading.
    ///   - content: The `KzSidebarItem`s.
    public init(_ title: String? = nil, @ViewBuilder action: () -> Action, @ViewBuilder content: () -> Content) {
        self.title = title
        self.action = action()
        self.content = content()
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let title {
                HStack(spacing: KzSpace.s2) {
                    Text(title)
                        .kzTextStyle(.caption, matchLineBox: false)
                        .foregroundStyle(KzColor.inkMuted)
                        .lineLimit(1)
                        .accessibilityAddTraits(.isHeader)
                    Spacer(minLength: 0)
                    action
                }
                .padding(.horizontal, 10)
                .frame(height: 22)
            }
            VStack(alignment: .leading, spacing: 1) { content }
        }
    }
}

extension KzSidebarSection where Action == EmptyView {
    /// Creates a section without a heading action.
    public init(_ title: String? = nil, @ViewBuilder content: () -> Content) {
        self.init(title, action: { EmptyView() }, content: content)
    }
}
