import SwiftUI

/// One snippet in the list pane: name, favorite star, group and use count on top; trigger chips and a
/// one-line preview of the content below.
///
/// It is a plain row, not a button: selection belongs to the list. In a `List(selection:)` mark the
/// row with `.kzListRow()` and pass `isSelected: selection.contains(id)`. See `kzSelectionList()`.
public struct KzSnippetRow: View {
    private let name: String
    private let triggers: [String]
    private let preview: String?
    private let group: String?
    private let uses: Int?
    private let isFavorite: Bool
    private let isSelected: Bool

    /// Creates a snippet row.
    /// - Parameters:
    ///   - name: The snippet's name (`body-strong`).
    ///   - triggers: Shown as chips, exactly as typed.
    ///   - preview: The expanded text, plain, shown on one line.
    ///   - group: The group name. Pass it only in cross-group views (All, Favorites, Recent).
    ///   - uses: The use count; hidden when `nil` or zero.
    ///   - isFavorite: Shows the 12px `bloom` star.
    ///   - isSelected: Selected rows get a `vine-soft` fill.
    public init(
        name: String, triggers: [String], preview: String? = nil, group: String? = nil,
        uses: Int? = nil, isFavorite: Bool = false, isSelected: Bool = false
    ) {
        self.name = name
        self.triggers = triggers
        self.preview = preview
        self.group = group
        self.uses = uses
        self.isFavorite = isFavorite
        self.isSelected = isSelected
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Text(name)
                    .kzTextStyle(.bodyStrong)
                    .foregroundStyle(KzColor.ink)
                    .lineLimit(1)
                if isFavorite {
                    KzIcon(.starFill, size: 12)
                        .foregroundStyle(KzColor.bloom)
                }
                Spacer(minLength: KzSpace.s2)
                if let group {
                    Text(group)
                        .kzTextStyle(.caption)
                        .foregroundStyle(KzColor.inkMuted)
                        .lineLimit(1)
                }
                if let uses, uses > 0 {
                    HStack(spacing: 2) {
                        KzIcon(.bolt, size: 11)
                        Text("\(uses)").kzTextStyle(.caption).monospacedDigit()
                    }
                    .foregroundStyle(KzColor.inkMuted)
                }
            }
            HStack(spacing: 6) {
                ForEach(triggers, id: \.self) { KzTriggerChip($0) }
                if let preview, !preview.isEmpty {
                    Text(preview)
                        .kzTextStyle(.callout)
                        .foregroundStyle(KzColor.inkMuted)
                        .lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: .leading)
                } else {
                    Spacer(minLength: 0)
                }
            }
        }
        .padding(.vertical, 10)
        .padding(.horizontal, KzSpace.s3)
        .frame(maxWidth: .infinity, alignment: .leading)
        .kzSelectableRow(isSelected: isSelected)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityTitle)
        .accessibilityValue(accessibilityDetail)
    }

    private var accessibilityTitle: String {
        isFavorite ? "\(name), favorite" : name
    }

    private var accessibilityDetail: String {
        var parts: [String] = []
        if !triggers.isEmpty { parts.append("Triggers " + triggers.joined(separator: ", ")) }
        if let group { parts.append("Group " + group) }
        if let uses, uses > 0 { parts.append("Used \(uses) \(uses == 1 ? "time" : "times")") }
        if let preview, !preview.isEmpty { parts.append(preview) }
        return parts.joined(separator: ". ")
    }
}
