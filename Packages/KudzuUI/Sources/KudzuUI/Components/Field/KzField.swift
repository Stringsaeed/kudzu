import SwiftUI

/// A labelled block in the snippet editor or a settings page: label on top, the control, an optional
/// hint below.
///
/// Labels are `headline` in `ink`; the aside and hint are `callout` in `ink-muted`. Stack fields
/// `space-5` apart, for example in a `KzFieldStack`. The label is linked to the control for
/// VoiceOver, so a control without a title of its own is still announced by the field's label.
public struct KzField<Control: View>: View {
    private let label: String
    private let aside: String?
    private let hint: String?
    private let control: Control
    @Namespace private var pairing

    /// Creates a field.
    /// - Parameters:
    ///   - label: The `headline` label ("Triggers").
    ///   - aside: A short note right of the label ("Space or return to add").
    ///   - hint: Help below the control.
    ///   - control: The control (`KzTextInput`, `KzTriggerInput`, `KzEditor`, …).
    public init(_ label: String, aside: String? = nil, hint: String? = nil, @ViewBuilder control: () -> Control) {
        self.label = label
        self.aside = aside
        self.hint = hint
        self.control = control()
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: KzSpace.s2) {
            HStack(alignment: .firstTextBaseline, spacing: KzSpace.s2) {
                Text(label)
                    .kzTextStyle(.headline)
                    .foregroundStyle(KzColor.ink)
                    .accessibilityLabeledPair(role: .label, id: "field", in: pairing)
                Spacer(minLength: 0)
                if let aside {
                    Text(aside)
                        .kzTextStyle(.callout)
                        .foregroundStyle(KzColor.inkMuted)
                }
            }
            control
                .accessibilityLabeledPair(role: .content, id: "field", in: pairing)
            if let hint {
                Text(hint)
                    .kzTextStyle(.callout)
                    .foregroundStyle(KzColor.inkMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// A vertical stack of fields, `space-5` apart.
public struct KzFieldStack<Content: View>: View {
    private let content: Content

    /// Creates the stack.
    public init(@ViewBuilder content: () -> Content) { self.content = content() }

    public var body: some View {
        VStack(alignment: .leading, spacing: KzSpace.s5) { content }
    }
}
