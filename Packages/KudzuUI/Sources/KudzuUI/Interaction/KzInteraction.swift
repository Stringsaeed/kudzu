import SwiftUI

/// The interaction state of a Kudzu control, handed to its content by `KzInteractive`.
public struct KzControlState: Equatable, Sendable {
    /// The pointer is over the control (always false when disabled).
    public var isHovered: Bool
    /// The control is being pressed (always false when disabled).
    public var isPressed: Bool
    /// The control has keyboard focus.
    public var isFocused: Bool
    /// The control is enabled.
    public var isEnabled: Bool

    /// Creates a state.
    public init(isHovered: Bool = false, isPressed: Bool = false, isFocused: Bool = false, isEnabled: Bool = true) {
        self.isHovered = isHovered
        self.isPressed = isPressed
        self.isFocused = isFocused
        self.isEnabled = isEnabled
    }

    /// The translucent overlay for this state: `surface-pressed`, `surface-hover` or nothing.
    public var overlay: Color? {
        guard isEnabled else { return nil }
        if isPressed { return KzColor.surfacePressed }
        if isHovered { return KzColor.surfaceHover }
        return nil
    }
}

/// How keyboard focus is drawn.
public enum KzFocusRingStyle: Sendable {
    /// Buttons and other controls: a 2px `focus` ring with a 2px gap outside the shape.
    case ring
    /// Fields: a 2px `vine-ink` edge centred on the shape's border.
    case edge
}

/// Forced interaction states for galleries and snapshots, which can't hover, press or focus.
public struct KzPreviewInteraction: Equatable, Sendable {
    /// Force the hovered look.
    public var hovered: Bool?
    /// Force the pressed look.
    public var pressed: Bool?
    /// Force the focus ring.
    public var focused: Bool?

    /// Creates an override; `nil` leaves that part of the state live.
    public init(hovered: Bool? = nil, pressed: Bool? = nil, focused: Bool? = nil) {
        self.hovered = hovered
        self.pressed = pressed
        self.focused = focused
    }
}

extension EnvironmentValues {
    /// Forced interaction state for previews; `KzInteractive` and `kzFocusRing` honour it.
    @Entry public var kzPreviewInteraction: KzPreviewInteraction?
    /// True while rendering offscreen (snapshots), where AppKit-backed views like materials can't draw.
    @Entry public var kzSnapshotRendering: Bool = false
}

extension View {
    /// Forces the hovered, pressed or focused look on Kudzu controls inside this view (galleries and snapshots only).
    public func kzPreviewInteraction(hovered: Bool? = nil, pressed: Bool? = nil, focused: Bool? = nil) -> some View {
        environment(\.kzPreviewInteraction, KzPreviewInteraction(hovered: hovered, pressed: pressed, focused: focused))
    }
}

// MARK: - Focus ring

private struct KzFocusRingModifier<S: InsettableShape>: ViewModifier {
    let shape: S
    let style: KzFocusRingStyle
    let isFocused: Bool?
    @Environment(\.isFocused) private var environmentFocused
    @Environment(\.kzPreviewInteraction) private var preview

    func body(content: Content) -> some View {
        let focused = preview?.focused ?? isFocused ?? environmentFocused
        content
            .focusEffectDisabled()
            .overlay {
                if focused {
                    switch style {
                    case .ring:
                        // 2px gap then a 2px ring: the stroke's centre line sits 3px outside the edge.
                        shape.inset(by: -3).stroke(KzColor.focus, lineWidth: 2)
                    case .edge:
                        shape.stroke(KzColor.vineInk, lineWidth: 2)
                    }
                }
            }
    }
}

extension View {
    /// Draws Kudzu's keyboard focus ring around `shape` and turns off the system focus effect.
    ///
    /// By default it follows `\.isFocused`, which on macOS is keyboard focus: controls only take
    /// focus by keyboard (Tab with Keyboard navigation on), so a click never shows the ring.
    /// Pass `isFocused` to drive it from a `@FocusState` instead (text fields).
    public func kzFocusRing<S: InsettableShape>(_ shape: S, style: KzFocusRingStyle = .ring, isFocused: Bool? = nil) -> some View {
        modifier(KzFocusRingModifier(shape: shape, style: style, isFocused: isFocused))
    }
}

// MARK: - Hover and press

private struct KzHoverModifier: ViewModifier {
    @Binding var isHovered: Bool
    @Environment(\.isEnabled) private var isEnabled

    func body(content: Content) -> some View {
        content
            .onHover { hovering in isHovered = hovering }
            .onChange(of: isEnabled) { _, enabled in if !enabled { isHovered = false } }
    }
}

extension View {
    /// Tracks pointer hover into `isHovered`, clearing it when the view is disabled.
    public func kzHover(_ isHovered: Binding<Bool>) -> some View {
        modifier(KzHoverModifier(isHovered: isHovered))
    }

    /// Lays the shared `surface-hover` / `surface-pressed` overlay over the view, clipped to `shape`.
    ///
    /// Nothing is drawn when the view is disabled. Colour changes use the 160 ms colour motion.
    public func kzStateLayer<S: Shape>(_ shape: S, isHovered: Bool, isPressed: Bool) -> some View {
        modifier(KzStateLayerModifier(shape: shape, isHovered: isHovered, isPressed: isPressed))
    }

    /// Applies the disabled treatment to text and icons: `ink-faint` when the view is disabled.
    public func kzDisabledForeground() -> some View {
        modifier(KzDisabledForegroundModifier())
    }
}

private struct KzStateLayerModifier<S: Shape>: ViewModifier {
    let shape: S
    let isHovered: Bool
    let isPressed: Bool
    @Environment(\.isEnabled) private var isEnabled

    func body(content: Content) -> some View {
        let state = KzControlState(isHovered: isHovered, isPressed: isPressed, isEnabled: isEnabled)
        content.overlay {
            shape.fill(state.overlay ?? .clear)
                .allowsHitTesting(false)
                .kzAnimation(.color, value: state)
        }
    }
}

private struct KzDisabledForegroundModifier: ViewModifier {
    @Environment(\.isEnabled) private var isEnabled

    func body(content: Content) -> some View {
        if isEnabled {
            content
        } else {
            content.foregroundStyle(KzDisabled.foreground)
        }
    }
}

/// The disabled treatment shared by every control.
public enum KzDisabled {
    /// Text and icons of a disabled control (`ink-faint`).
    public static let foreground: Color = KzColor.inkFaint
    /// Fill of a disabled control that is normally filled, such as the primary button (`surface-sunken`).
    public static let fill: Color = KzColor.surfaceSunken
}

// MARK: - KzInteractive

/// The building block for every Kudzu control's body: tracks hover, focus and enabled state,
/// draws the shared hover/press layer and focus ring, and animates colour with the motion token.
///
/// Use it inside a `ButtonStyle` / `ToggleStyle` so the native control keeps its semantics:
///
/// ```swift
/// struct MyStyle: ButtonStyle {
///     func makeBody(configuration: Configuration) -> some View {
///         KzInteractive(RoundedRectangle(cornerRadius: KzRadius.md), isPressed: configuration.isPressed) { state in
///             configuration.label
///                 .foregroundStyle(state.isEnabled ? KzColor.ink : KzDisabled.foreground)
///         }
///     }
/// }
/// ```
public struct KzInteractive<S: InsettableShape, Content: View>: View {
    private let shape: S
    private let isPressed: Bool
    private let stateLayer: Bool
    private let focusRing: KzFocusRingStyle?
    private let isFocusedOverride: Bool?
    private let content: (KzControlState) -> Content

    @State private var isHovered = false
    @Environment(\.isEnabled) private var isEnabled
    @Environment(\.isFocused) private var isFocusedEnvironment
    @Environment(\.kzPreviewInteraction) private var preview

    /// Creates an interactive surface.
    /// - Parameters:
    ///   - shape: The control's outline; hover layer, focus ring and hit area follow it.
    ///   - isPressed: Usually `configuration.isPressed`.
    ///   - stateLayer: Draw the shared hover/press overlay. Turn off when the control changes its own fill instead (primary button).
    ///   - focusRing: How to draw keyboard focus, or `nil` when an ancestor draws it.
    ///   - isFocused: Override focus (from a `@FocusState`); defaults to `\.isFocused`.
    public init(_ shape: S, isPressed: Bool = false, stateLayer: Bool = true, focusRing: KzFocusRingStyle? = .ring, isFocused: Bool? = nil, @ViewBuilder content: @escaping (KzControlState) -> Content) {
        self.shape = shape
        self.isPressed = isPressed
        self.stateLayer = stateLayer
        self.focusRing = focusRing
        self.isFocusedOverride = isFocused
        self.content = content
    }

    private var state: KzControlState {
        KzControlState(
            isHovered: isEnabled && (preview?.hovered ?? isHovered),
            isPressed: isEnabled && (preview?.pressed ?? isPressed),
            isFocused: preview?.focused ?? isFocusedOverride ?? isFocusedEnvironment,
            isEnabled: isEnabled
        )
    }

    public var body: some View {
        let state = state
        content(state)
            .overlay {
                if stateLayer {
                    shape.fill(state.overlay ?? .clear).allowsHitTesting(false)
                }
            }
            .contentShape(shape)
            .kzHover($isHovered)
            .overlay {
                if let focusRing, state.isFocused {
                    switch focusRing {
                    case .ring: shape.inset(by: -3).stroke(KzColor.focus, lineWidth: 2)
                    case .edge: shape.stroke(KzColor.vineInk, lineWidth: 2)
                    }
                }
            }
            .focusEffectDisabled()
            .kzAnimation(.color, value: state)
    }
}
