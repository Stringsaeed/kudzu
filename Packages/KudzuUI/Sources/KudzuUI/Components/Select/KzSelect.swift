import SwiftUI

/// A pop-up command showing the current label in a TextInput well.
public struct KzSelect: View {
    private let value: String
    private let onOpen: () -> Void
    @Environment(\.isEnabled) private var enabled
    /// `onOpen` presents the consumer's menu (including a future KzMenu); this view owns no menu state.
    public init(_ value: String, onOpen: @escaping () -> Void) { self.value = value; self.onOpen = onOpen }
    public var body: some View {
        Button(action: onOpen) {
            HStack(spacing: 8) {
                Text(value).kzTextStyle(.bodyStrong, matchLineBox: false)
                Spacer(minLength: 0)
                KzIcon(.chevronsV, size: 14).foregroundStyle(enabled ? KzColor.inkMuted : KzDisabled.foreground)
            }.padding(.leading, 10).padding(.trailing, 6).frame(minWidth: 140).frame(height: 30)
        }.buttonStyle(Style()).accessibilityLabel(value).accessibilityHint("Opens choices")
    }
    private struct Style: ButtonStyle {
        func makeBody(configuration: Configuration) -> some View {
            let shape = RoundedRectangle(cornerRadius: KzRadius.md)
            KzInteractive(shape, isPressed: configuration.isPressed) { state in
                configuration.label.foregroundStyle(state.isEnabled ? KzColor.ink : KzDisabled.foreground)
                    .background(KzColor.surfaceSunken, in: shape)
                    .overlay(shape.strokeBorder(state.isHovered ? KzColor.line : .clear))
            }
        }
    }
}
