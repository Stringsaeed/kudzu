import SwiftUI

/// One setting: label and optional description on the left, its control on the right.
///
/// Put rows inside a `KzSettingsGroup`, which splits them with `line` hairlines. The row is at least
/// 44pt tall with 16pt side padding.
public struct KzSettingsRow<Label: View, Control: View>: View {
    private let icon: KzIconName?
    private let descriptionText: String?
    private let label: Label
    private let control: Control

    /// Creates a row with a custom label view (for example a title followed by a `KzBadge`).
    /// - Parameters:
    ///   - description: One sentence in `callout`, `ink-muted`; no trailing period needed for fragments.
    ///   - icon: A 16pt `ink-muted` glyph before the label.
    ///   - label: The label content, set in `body-strong`.
    ///   - control: `KzToggle`, `KzSegmented`, `KzStepper`, `KzSelect` or a small `KzButton`.
    public init(description: String? = nil, icon: KzIconName? = nil, @ViewBuilder label: () -> Label, @ViewBuilder control: () -> Control) {
        self.descriptionText = description
        self.icon = icon
        self.label = label()
        self.control = control()
    }

    public var body: some View {
        HStack(spacing: KzSpace.s3) {
            if let icon {
                KzIcon(icon, size: 16).foregroundStyle(KzColor.inkMuted)
            }
            VStack(alignment: .leading, spacing: 2) {
                label
                    .kzTextStyle(.bodyStrong)
                    .foregroundStyle(KzColor.ink)
                    .lineLimit(2)
                if let descriptionText {
                    Text(descriptionText)
                        .kzTextStyle(.callout)
                        .foregroundStyle(KzColor.inkMuted)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .layoutPriority(1)
            .frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
            HStack(spacing: KzSpace.s2) { control }
                .frame(minWidth: 0, alignment: .trailing)
        }
        .padding(.horizontal, KzSpace.s4)
        .padding(.vertical, 8)
        .frame(minHeight: 44)
        .accessibilityElement(children: .contain)
    }
}

extension KzSettingsRow where Label == Text {
    /// Creates a row with a text label.
    public init(_ label: String, description: String? = nil, icon: KzIconName? = nil, @ViewBuilder control: () -> Control) {
        self.init(description: description, icon: icon, label: { Text(label) }, control: control)
    }
}
