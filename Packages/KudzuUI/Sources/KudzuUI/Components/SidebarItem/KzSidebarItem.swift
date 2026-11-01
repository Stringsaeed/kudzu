import SwiftUI

/// A row in the sidebar (and in the settings section list): icon, label, optional count or shortcut.
///
/// A real `Button`, so it takes keyboard focus and activation. The active item is a solid
/// `glass-selected` pill with `shadow-control` and a `vine-ink` icon, and carries the selected
/// accessibility trait. There is no full-colour selection.
public struct KzSidebarItem: View {
    private let label: String
    private let icon: KzIconName
    private let count: Int?
    private let shortcut: String?
    private let isActive: Bool
    private let action: () -> Void

    /// Creates a sidebar item.
    /// - Parameters:
    ///   - label: Sentence case ("Recently used").
    ///   - icon: Drawn at 16pt, `ink-muted`, or `vine-ink` when active.
    ///   - count: A bare number in `caption`, tabular.
    ///   - shortcut: Mac glyphs in a `KzKbd` ("⌘,"). Used by Settings in the sidebar footer.
    ///   - isActive: Marks the item as the current destination.
    ///   - action: Runs when the item is chosen.
    public init(
        _ label: String, icon: KzIconName, count: Int? = nil, shortcut: String? = nil,
        isActive: Bool = false, action: @escaping () -> Void
    ) {
        self.label = label
        self.icon = icon
        self.count = count
        self.shortcut = shortcut
        self.isActive = isActive
        self.action = action
    }

    public var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                KzIcon(icon, size: 16)
                    .foregroundStyle(isActive ? KzColor.vineInk : KzColor.inkMuted)
                Text(label)
                    .kzTextStyle(.bodyStrong, matchLineBox: false)
                    .foregroundStyle(KzColor.ink)
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                if let count {
                    Text("\(count)")
                        .kzTextStyle(.caption, matchLineBox: false)
                        .monospacedDigit()
                        .foregroundStyle(KzColor.inkMuted)
                }
                if let shortcut {
                    KzKbd(shortcut)
                }
            }
        }
        .buttonStyle(KzSidebarItemStyle(isActive: isActive))
        .accessibilityAddTraits(isActive ? .isSelected : [])
    }
}

/// The sidebar item's button style: a 30pt pill with `radius-md`, hover from `surface-hover`, and a
/// raised `glass-selected` fill when active.
public struct KzSidebarItemStyle: ButtonStyle {
    private let isActive: Bool

    /// Creates the style.
    public init(isActive: Bool = false) { self.isActive = isActive }

    public func makeBody(configuration: Configuration) -> some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.md)
        // An active item keeps its solid pill under the pointer, as in the CSS.
        KzInteractive(shape, isPressed: configuration.isPressed, stateLayer: !isActive) { _ in
            let pill = configuration.label
                .padding(.horizontal, 10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .frame(height: KzSize.control)
                .background(isActive ? KzColor.glassSelected : .clear, in: shape)
            if isActive {
                pill.kzShadow(.control, in: shape)
            } else {
                pill
            }
        }
    }
}
