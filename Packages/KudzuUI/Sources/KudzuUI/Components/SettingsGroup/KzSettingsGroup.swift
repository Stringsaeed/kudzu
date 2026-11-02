import SwiftUI

/// A titled card of settings rows on a settings page.
///
/// The card is `surface-raised` with a `line` border and `radius-lg`; rows inside are split by `line`
/// hairlines. Stack groups `space-6` apart on the `surface` page.
public struct KzSettingsGroup<Action: View, Content: View>: View {
    private let title: String?
    private let descriptionText: String?
    private let action: Action
    private let content: Content

    /// Creates a group.
    /// - Parameters:
    ///   - title: The `headline` title ("Backups").
    ///   - description: One short sentence under the title, at most 440pt wide.
    ///   - action: A small `KzButton` right of the title ("Back up now").
    ///   - content: `KzSettingsRow`s. Each child gets a hairline above it.
    public init(_ title: String? = nil, description: String? = nil, @ViewBuilder action: () -> Action, @ViewBuilder content: () -> Content) {
        self.title = title
        self.descriptionText = description
        self.action = action()
        self.content = content()
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if title != nil || descriptionText != nil {
                HStack(alignment: .bottom, spacing: KzSpace.s4) {
                    VStack(alignment: .leading, spacing: 2) {
                        if let title {
                            Text(title)
                                .kzTextStyle(.headline)
                                .foregroundStyle(KzColor.ink)
                                .accessibilityAddTraits(.isHeader)
                        }
                        if let descriptionText {
                            Text(descriptionText)
                                .kzTextStyle(.callout)
                                .foregroundStyle(KzColor.inkMuted)
                                .frame(maxWidth: 440, alignment: .leading)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                    Spacer(minLength: 0)
                    action
                }
                .padding(.horizontal, 2)
            }
            card
        }
    }

    private var card: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.lg)
        return Group(subviews: content) { rows in
            VStack(spacing: 0) {
                ForEach(Array(rows.enumerated()), id: \.element.id) { index, row in
                    if index > 0 { KzHairline() }
                    row
                }
            }
        }
        .frame(maxWidth: .infinity)
        .background(KzColor.surfaceRaised, in: shape)
        .clipShape(shape)
        .overlay(shape.strokeBorder(KzColor.line))
        .accessibilityElement(children: .contain)
    }
}

extension KzSettingsGroup where Action == EmptyView {
    /// Creates a group without a header action.
    public init(_ title: String? = nil, description: String? = nil, @ViewBuilder content: () -> Content) {
        self.init(title, description: description, action: { EmptyView() }, content: content)
    }
}
