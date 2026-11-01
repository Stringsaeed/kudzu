import SwiftUI

/// A labelled native icon command for toolbars and pane headers.
public struct KzIconButton: View {
    /// Bloom is reserved for the favorite star; other icons remain neutral.
    public enum Tone: Sendable { case neutral, bloom }
    private let icon: KzIconName
    private let label: String
    private let menu: Bool
    private let active: Bool
    private let size: KzButton.Size
    private let tone: Tone
    private let action: () -> Void
    /// Creates an icon button; `label` supplies both tooltip and accessible name.
    public init(_ icon: KzIconName, label: String, menu: Bool = false, active: Bool = false,
                size: KzButton.Size = .md, tone: Tone = .neutral, action: @escaping () -> Void) {
        self.icon = icon; self.label = label; self.menu = menu; self.active = active
        self.size = size; self.tone = tone; self.action = action
    }
    public var body: some View {
        Button(action: action) {
            HStack(spacing: 2) {
                KzIcon(icon, size: size == .sm ? 14 : 16)
                if menu { KzIcon(.chevronDown, size: 10).opacity(0.7) }
            }
        }
        .buttonStyle(Style(active: active, size: size, menu: menu, bloom: tone == .bloom && icon == .starFill))
        .help(label).accessibilityLabel(label)
        .accessibilityAddTraits(active ? .isSelected : [])
    }
    private struct Style: ButtonStyle {
        let active: Bool
        let size: KzButton.Size
        let menu: Bool
        let bloom: Bool
        func makeBody(configuration: Configuration) -> some View {
            let shape = RoundedRectangle(cornerRadius: size == .sm ? KzRadius.sm : KzRadius.md)
            KzInteractive(shape, isPressed: configuration.isPressed, stateLayer: !active) { state in
                configuration.label
                    .foregroundStyle(!state.isEnabled ? KzDisabled.foreground : bloom ? KzColor.bloom : active ? KzColor.vineInk : state.isHovered ? KzColor.ink : KzColor.inkMuted)
                    .padding(.leading, menu ? 8 : 0).padding(.trailing, menu ? 6 : 0)
                    .frame(width: menu ? nil : size == .sm ? 24 : 30, height: size == .sm ? 24 : 30)
                    .background(active ? (state.isEnabled ? KzColor.vineSoft : KzDisabled.fill) : .clear, in: shape)
            }
        }
    }
}
