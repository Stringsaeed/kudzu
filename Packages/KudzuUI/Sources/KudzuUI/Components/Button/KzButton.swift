import SwiftUI

/// A native command button with Kudzu's visual variants.
public struct KzButton: View {
    /// Command emphasis; use at most one primary per view.
    public enum Variant: CaseIterable, Sendable { case primary, secondary, ghost, danger }
    /// CSS control heights: medium 30pt, small 24pt.
    public enum Size: CaseIterable, Sendable { case md, sm }
    private let title: String
    private let variant: Variant
    private let size: Size
    private let icon: KzIconName?
    private let iconEnd: KzIconName?
    private let isDefault: Bool
    private let action: (() -> Void)?
    private let menu: (title: String?, placement: KzMenuPlacement, content: () -> [KzMenuElement])?

    /// Creates a command; `isDefault` opts into native Return activation.
    public init(_ title: String, variant: Variant = .secondary, size: Size = .md,
                icon: KzIconName? = nil, iconEnd: KzIconName? = nil,
                isDefault: Bool = false, action: @escaping () -> Void) {
        self.title = title; self.variant = variant; self.size = size
        self.icon = icon; self.iconEnd = iconEnd; self.isDefault = isDefault; self.action = action; self.menu = nil
    }

    /// Creates a button that opens a Kudzu menu instead of running an action (the editor's
    /// "Insert", for example), built on `KzMenuButton`: it opens on mouse-down and supports
    /// press-drag-release. Pass `iconEnd: .chevronDown` to mark it as a pull-down.
    public init(_ title: String, variant: Variant = .secondary, size: Size = .md,
                icon: KzIconName? = nil, iconEnd: KzIconName? = nil,
                menuTitle: String? = nil, placement: KzMenuPlacement = .below,
                @KzMenuBuilder menu: @escaping () -> [KzMenuElement]) {
        self.title = title; self.variant = variant; self.size = size
        self.icon = icon; self.iconEnd = iconEnd; self.isDefault = false; self.action = nil
        self.menu = (menuTitle, placement, menu)
    }

    public var body: some View {
        if let menu {
            KzMenuButton(title: menu.title, placement: menu.placement, menu: menu.content) { _ in content }
                .buttonStyle(KzButtonStyle(variant: variant, size: size))
        } else if isDefault {
            button.keyboardShortcut(.defaultAction)
        } else {
            button
        }
    }

    private var button: some View {
        Button(action: action ?? {}) { content }
            .buttonStyle(KzButtonStyle(variant: variant, size: size))
    }

    private var content: some View {
        HStack(spacing: size == .sm ? 4 : 6) {
            if let icon { KzIcon(icon, size: size == .sm ? 14 : 16) }
            Text(title).fixedSize().kzTextStyle(size == .sm ? .callout : .bodyStrong, matchLineBox: false).fontWeight(.medium)
            if let iconEnd { KzIcon(iconEnd, size: 12).opacity(0.8).padding(.leading, -2) }
        }
    }
}

/// Kudzu styling for an existing native Button and its custom label.
public struct KzButtonStyle: ButtonStyle {
    private let variant: KzButton.Variant
    private let size: KzButton.Size
    /// Creates the style without changing native button semantics.
    public init(variant: KzButton.Variant = .secondary, size: KzButton.Size = .md) {
        self.variant = variant; self.size = size
    }
    public func makeBody(configuration: Configuration) -> some View {
        let shape = RoundedRectangle(cornerRadius: size == .sm ? KzRadius.sm : KzRadius.md)
        KzInteractive(shape, isPressed: configuration.isPressed, stateLayer: variant != .primary) { state in
            configuration.label
                .foregroundStyle(ink(state))
                .padding(.horizontal, size == .sm ? 8 : 12)
                .frame(height: size == .sm ? 24 : 30)
                .background(fill(state), in: shape)
                .overlay(shape.strokeBorder(variant == .primary || variant == .ghost ? .clear : KzColor.line))
                .overlay { if variant == .primary && state.isPressed { shape.fill(KzColor.surfacePressed).allowsHitTesting(false) } }
                .background { if variant != .ghost { shape.fill(.clear).kzShadow(.control, in: shape) } }
        }
    }
    private func ink(_ state: KzControlState) -> Color {
        guard state.isEnabled else { return KzDisabled.foreground }
        switch variant {
        case .primary: return KzColor.onVine
        case .danger: return KzColor.danger
        case .ghost: return state.isHovered ? KzColor.ink : KzColor.inkMuted
        case .secondary: return KzColor.ink
        }
    }
    private func fill(_ state: KzControlState) -> Color {
        guard state.isEnabled else { return variant == .ghost ? .clear : KzDisabled.fill }
        switch variant {
        case .primary: return state.isHovered || state.isPressed ? KzColor.vineHover : KzColor.vine
        case .ghost: return .clear
        default: return KzColor.surfaceRaised
        }
    }
}
