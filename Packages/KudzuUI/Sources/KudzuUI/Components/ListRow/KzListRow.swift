import SwiftUI

/// A generic two-line list row for non-snippet lists: backups, the stats ranking.
///
/// Like `KzSnippetRow` it is a plain row; selection belongs to the list. Selection and hover match
/// `KzSnippetRow`. Use `.kzListRow()` inside a `List` and pass `isSelected`.
public struct KzListRow<Title: View, Badge: View>: View {
    private let title: Title
    private let meta: String?
    private let aside: String?
    private let badge: Badge
    private let isSelected: Bool
    private let accessibilityName: String?

    /// Creates a row whose title is any view, typically a `KzTriggerChip`.
    /// - Parameters:
    ///   - meta: Set in `mono-caption`: file names, timestamps.
    ///   - aside: A count (`caption`, tabular).
    ///   - isSelected: Selected rows get a `vine-soft` fill.
    ///   - accessibilityName: What VoiceOver should call the row when the title is not text.
    ///   - title: The first line.
    ///   - badge: A trailing `KzBadge`.
    public init(
        meta: String? = nil, aside: String? = nil, isSelected: Bool = false, accessibilityName: String? = nil,
        @ViewBuilder title: () -> Title, @ViewBuilder badge: () -> Badge
    ) {
        self.title = title()
        self.meta = meta
        self.aside = aside
        self.badge = badge()
        self.isSelected = isSelected
        self.accessibilityName = accessibilityName
    }

    public var body: some View {
        HStack(spacing: KzSpace.s3) {
            VStack(alignment: .leading, spacing: 2) {
                title
                if let meta {
                    Text(meta)
                        .kzTextStyle(.monoCaption)
                        .foregroundStyle(KzColor.inkMuted)
                        .lineLimit(1)
                        .truncationMode(.tail)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if let aside {
                Text(aside)
                    .kzTextStyle(.caption)
                    .monospacedDigit()
                    .foregroundStyle(KzColor.inkMuted)
                    .lineLimit(1)
                    .fixedSize()
            }
            badge
        }
        .padding(.vertical, 10)
        .padding(.horizontal, KzSpace.s3)
        .frame(maxWidth: .infinity, alignment: .leading)
        .kzSelectableRow(isSelected: isSelected)
        .accessibilityElement(children: .combine)
        .modifier(OptionalLabel(label: accessibilityName))
    }

    private struct OptionalLabel: ViewModifier {
        let label: String?
        func body(content: Content) -> some View {
            if let label { content.accessibilityLabel(label) } else { content }
        }
    }
}

/// The text title of a `KzListRow` (`body-strong`, one line).
public struct KzListRowTitle: View {
    private let text: String

    /// Creates a title.
    public init(_ text: String) { self.text = text }

    public var body: some View {
        Text(text).kzTextStyle(.bodyStrong).foregroundStyle(KzColor.ink).lineLimit(1)
    }
}

extension KzListRow where Title == KzListRowTitle {
    /// Creates a row with a text title (`body-strong`) and a trailing badge.
    public init(
        _ title: String, meta: String? = nil, aside: String? = nil, isSelected: Bool = false,
        @ViewBuilder badge: () -> Badge
    ) {
        self.init(meta: meta, aside: aside, isSelected: isSelected, title: { KzListRowTitle(title) }, badge: badge)
    }
}

extension KzListRow where Badge == EmptyView {
    /// Creates a row with a custom title view and no badge.
    public init(
        meta: String? = nil, aside: String? = nil, isSelected: Bool = false, accessibilityName: String? = nil,
        @ViewBuilder title: () -> Title
    ) {
        self.init(meta: meta, aside: aside, isSelected: isSelected, accessibilityName: accessibilityName, title: title, badge: { EmptyView() })
    }
}

extension KzListRow where Title == KzListRowTitle, Badge == EmptyView {
    /// Creates a row with a text title and no badge.
    public init(_ title: String, meta: String? = nil, aside: String? = nil, isSelected: Bool = false) {
        self.init(meta: meta, aside: aside, isSelected: isSelected, title: { KzListRowTitle(title) }, badge: { EmptyView() })
    }
}
