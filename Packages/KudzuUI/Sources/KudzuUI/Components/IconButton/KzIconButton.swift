import SwiftUI

/// A labelled native icon command for toolbars and pane headers.
///
/// There are two forms. One runs an action:
///
/// ```swift
/// KzIconButton(.compose, label: "New snippet") { newSnippet() }
/// ```
///
/// The other opens a Kudzu menu (sort, "more"), built on `KzMenuButton`. It opens on mouse-down, shows
/// the `active` look (`vine-soft` fill, `vine-ink` icon) while the menu is open, and takes a caret
/// with `caret: true` to say it's a pop-up:
///
/// ```swift
/// KzIconButton(.sort, label: "Sort", caret: true, menuTitle: "Sort by") {
///     for order in SortOrder.allCases { KzMenuItem(order.title, isChecked: sort == order) { sort = order } }
/// }
/// ```
///
/// Inside a `KzToolbarGroup` (`\.kzInToolbarGroup`) buttons are 28pt tall instead of 30.
public struct KzIconButton: View {
    /// Bloom is reserved for the favorite star; other icons remain neutral.
    public enum Tone: Sendable { case neutral, bloom }

    private enum Kind {
        case action(() -> Void)
        case menu(title: String?, placement: KzMenuPlacement, content: () -> [KzMenuElement])
    }

    private let icon: KzIconName
    private let label: String
    private let caret: Bool
    private let active: Bool
    private let size: KzButton.Size
    private let tone: Tone
    private let kind: Kind
    @State private var isMenuOpen = false

    /// Creates an icon button that runs `action`; `label` supplies both tooltip and accessible name.
    /// `caret` adds the down chevron, for buttons that open a menu through `.kzMenu(isPresented:)`.
    public init(_ icon: KzIconName, label: String, caret: Bool = false, active: Bool = false,
                size: KzButton.Size = .md, tone: Tone = .neutral, action: @escaping () -> Void) {
        self.icon = icon; self.label = label; self.caret = caret; self.active = active
        self.size = size; self.tone = tone; self.kind = .action(action)
    }

    /// Creates an icon button that opens a menu; `label` supplies both tooltip and accessible name.
    /// - Parameters:
    ///   - caret: Adds the down chevron (`opacity .7`) that marks a pop-up, like Sort.
    ///   - active: Draws the toggled look even while the menu is closed. It is always drawn while open.
    ///   - menuTitle: The `caption` title at the top of the menu.
    ///   - placement: Where the menu opens; below the button by default.
    ///   - menu: The items, built when the menu opens.
    public init(_ icon: KzIconName, label: String, caret: Bool = false, active: Bool = false,
                size: KzButton.Size = .md, tone: Tone = .neutral, menuTitle: String? = nil,
                placement: KzMenuPlacement = .below, @KzMenuBuilder menu: @escaping () -> [KzMenuElement]) {
        self.icon = icon; self.label = label; self.caret = caret; self.active = active
        self.size = size; self.tone = tone; self.kind = .menu(title: menuTitle, placement: placement, content: menu)
    }

    public var body: some View {
        Group {
            switch kind {
            case .action(let action):
                Button(action: action) { content }
                    .buttonStyle(style(isActive: active))
            case .menu(let title, let placement, let items):
                KzMenuButton(title: title, placement: placement, menu: items) { isOpen in
                    content.onChange(of: isOpen, initial: true) { _, open in isMenuOpen = open }
                }
                .buttonStyle(style(isActive: active || isMenuOpen))
            }
        }
        .help(label)
        .accessibilityLabel(label)
        .accessibilityAddTraits(active ? .isSelected : [])
    }

    private var content: some View {
        HStack(spacing: 2) {
            KzIcon(icon, size: size == .sm ? 14 : 16)
            if caret { KzIcon(.chevronDown, size: 10).opacity(0.7) }
        }
    }

    private func style(isActive: Bool) -> Style {
        Style(active: isActive, size: size, caret: caret, bloom: tone == .bloom && icon == .starFill)
    }

    private struct Style: ButtonStyle {
        let active: Bool
        let size: KzButton.Size
        let caret: Bool
        let bloom: Bool
        @Environment(\.kzInToolbarGroup) private var inToolbar

        func makeBody(configuration: Configuration) -> some View {
            let shape = RoundedRectangle(cornerRadius: size == .sm ? KzRadius.sm : KzRadius.md)
            let height: CGFloat = inToolbar ? 28 : size == .sm ? KzSize.controlSm : KzSize.control
            KzInteractive(shape, isPressed: configuration.isPressed, stateLayer: !active) { state in
                configuration.label
                    .foregroundStyle(!state.isEnabled ? KzDisabled.foreground : bloom ? KzColor.bloom : active ? KzColor.vineInk : state.isHovered ? KzColor.ink : KzColor.inkMuted)
                    .padding(.leading, caret ? 8 : 0).padding(.trailing, caret ? 6 : 0)
                    .frame(width: caret ? nil : size == .sm ? KzSize.controlSm : KzSize.control, height: height)
                    .background(active ? (state.isEnabled ? KzColor.vineSoft : KzDisabled.fill) : .clear, in: shape)
            }
        }
    }
}
