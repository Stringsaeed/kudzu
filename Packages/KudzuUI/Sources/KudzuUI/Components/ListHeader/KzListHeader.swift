import SwiftUI

/// The header row of the list pane: title, count, and the list's own actions (sort, new snippet).
///
/// Its height is `size-titlebar`, so it lines up with the traffic lights in the sidebar.
public struct KzListHeader<Actions: View>: View {
    private let title: String
    private let subtitle: String?
    private let actions: Actions

    /// Creates a header.
    /// - Parameters:
    ///   - title: The pane title (`title`).
    ///   - subtitle: A count in lowercase units ("18 snippets").
    ///   - actions: `KzIconButton`s, laid out 2px apart.
    public init(_ title: String, subtitle: String? = nil, @ViewBuilder actions: () -> Actions) {
        self.title = title
        self.subtitle = subtitle
        self.actions = actions()
    }

    public var body: some View {
        HStack(spacing: KzSpace.s2) {
            VStack(alignment: .leading, spacing: 0) {
                Text(title)
                    .kzTextStyle(.title)
                    .foregroundStyle(KzColor.ink)
                    .lineLimit(1)
                    .accessibilityAddTraits(.isHeader)
                if let subtitle {
                    Text(subtitle)
                        .kzTextStyle(.caption)
                        .foregroundStyle(KzColor.inkMuted)
                        .lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 2) { actions }
        }
        .padding(.leading, KzSpace.s4)
        .padding(.trailing, KzSpace.s3)
        .frame(minHeight: KzSize.titlebar)
        .kzWindowDragArea()
    }
}

extension KzListHeader where Actions == EmptyView {
    /// Creates a header without actions.
    public init(_ title: String, subtitle: String? = nil) {
        self.init(title, subtitle: subtitle, actions: { EmptyView() })
    }
}
