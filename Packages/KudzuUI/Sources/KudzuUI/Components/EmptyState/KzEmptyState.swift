import SwiftUI

/// What a pane shows when it has nothing to show: a faint leaf mark, a short title, one sentence
/// that says what to do (with the shortcut), and optionally an action.
public struct KzEmptyState<Message: View, Action: View>: View {
    private let title: String
    private let icon: KzIconName
    private let message: Message
    private let action: Action

    /// Creates an empty state.
    /// - Parameters:
    ///   - title: `title-lg`, e.g. "No snippet selected".
    ///   - icon: The 28pt `ink-faint` mark; defaults to the leaf.
    ///   - message: The sentence, `ink-muted`, at most 280pt wide. Use `KzMessage` to include a `KzKbd`.
    ///   - action: A `KzButton`.
    public init(_ title: String, icon: KzIconName = .leaf, @ViewBuilder message: () -> Message, @ViewBuilder action: () -> Action) {
        self.title = title
        self.icon = icon
        self.message = message()
        self.action = action()
    }

    public var body: some View {
        VStack(spacing: KzSpace.s2) {
            // 8px gap plus the CSS 8px bottom margin on the mark.
            KzIcon(icon, size: 28)
                .foregroundStyle(KzColor.inkFaint)
                .padding(.bottom, KzSpace.s2)
            Text(title)
                .kzTextStyle(.titleLg)
                .foregroundStyle(KzColor.ink)
                .multilineTextAlignment(.center)
                .accessibilityAddTraits(.isHeader)
            message
                .kzTextStyle(.body)
                .foregroundStyle(KzColor.inkMuted)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 280)
                .fixedSize(horizontal: false, vertical: true)
            action
        }
        .padding(.vertical, KzSpace.s8)
        .padding(.horizontal, KzSpace.s6)
        .accessibilityElement(children: .contain)
    }
}

extension KzEmptyState where Action == EmptyView {
    /// Creates an empty state without an action.
    public init(_ title: String, icon: KzIconName = .leaf, @ViewBuilder message: () -> Message) {
        self.init(title, icon: icon, message: message, action: { EmptyView() })
    }
}

extension KzEmptyState where Message == Text {
    /// Creates an empty state with a plain-text sentence.
    public init(_ title: String, icon: KzIconName = .leaf, message: String, @ViewBuilder action: () -> Action) {
        self.init(title, icon: icon, message: { Text(message) }, action: action)
    }
}

extension KzEmptyState where Message == Text, Action == EmptyView {
    /// Creates an empty state with a plain-text sentence and no action.
    public init(_ title: String, icon: KzIconName = .leaf, message: String) {
        self.init(title, icon: icon, message: { Text(message) }, action: { EmptyView() })
    }
}
