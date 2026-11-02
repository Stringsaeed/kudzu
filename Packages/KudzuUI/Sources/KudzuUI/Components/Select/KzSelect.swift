import SwiftUI

/// A pop-up button: a `surface-sunken` well showing the current choice that opens Kudzu's glass
/// menu with the checked item over the field, like `NSPopUpButton`.
///
/// ```swift
/// KzSelect("Group", selection: $group, options: groups.map { ($0.id, $0.name) })
/// ```
///
/// It is a `KzMenuButton(placement: .select)` styled with `KzSelectStyle`: the menu opens on
/// mouse-down (press, drag and release chooses), Space opens it from the keyboard, and arrows,
/// type-ahead and Esc work as in any Mac menu. For choices that aren't a plain list of values
/// (headers, icons, submenus), use `init(_:value:placement:menu:)`.
public struct KzSelect: View {
    private let label: String
    private let display: String
    private let isPlaceholder: Bool
    private let placement: KzMenuPlacement
    private let menu: () -> [KzMenuElement]

    /// Creates a select over a list of values.
    /// - Parameters:
    ///   - label: Names the control for VoiceOver ("Group"). It is shown only while `selection` matches no option.
    ///   - selection: The chosen value; the matching option is checked in the menu.
    ///   - options: Each value with the text shown for it, in menu order.
    public init<Value: Hashable>(_ label: String, selection: Binding<Value>, options: [(Value, String)]) {
        let current = options.first { $0.0 == selection.wrappedValue }
        self.label = label
        self.display = current?.1 ?? label
        self.isPlaceholder = current == nil
        self.placement = .select
        self.menu = {
            options.map { option in
                .item(KzMenuItem(option.1, isChecked: option.0 == selection.wrappedValue, id: "\(option.0)") {
                    selection.wrappedValue = option.0
                })
            }
        }
    }

    /// Creates a select that shows `value` and opens an arbitrary menu: the lower-level form of
    /// `init(_:selection:options:)`. Mark the current item with `isChecked: true` so a pop-up
    /// menu puts it over the field.
    public init(_ label: String, value: String, placement: KzMenuPlacement = .select,
                @KzMenuBuilder menu: @escaping () -> [KzMenuElement]) {
        self.label = label
        self.display = value
        self.isPlaceholder = false
        self.placement = placement
        self.menu = menu
    }

    /// The menu this select opens, for tests.
    var menuElements: [KzMenuElement] { menu() }
    var displayedValue: String { display }

    public var body: some View {
        KzMenuButton(placement: placement, menu: menu) { isOpen in
            KzSelectLabel(display, isOpen: isOpen, isPlaceholder: isPlaceholder)
        }
        .buttonStyle(KzSelectStyle())
        .accessibilityLabel(label)
        .accessibilityValue(display)
    }
}

/// The inside of a select (`.kz-select`): the value in `body-strong`, up-down chevrons in `ink-muted`,
/// 10pt from the leading edge (where a pop-up menu aligns its checked item), at least 140pt wide.
///
/// Use inside `KzMenuButton(placement: .select)` with `.buttonStyle(KzSelectStyle())` when `KzSelect`
/// doesn't fit. While `isOpen` the well keeps its hover border, as a pressed pop-up does.
public struct KzSelectLabel: View {
    private let value: String
    private let isOpen: Bool
    private let isPlaceholder: Bool
    @Environment(\.isEnabled) private var isEnabled

    /// Creates the label for the current `value`.
    public init(_ value: String, isOpen: Bool = false, isPlaceholder: Bool = false) {
        self.value = value
        self.isOpen = isOpen
        self.isPlaceholder = isPlaceholder
    }

    public var body: some View {
        HStack(spacing: KzSpace.s2) {
            Text(value)
                .kzTextStyle(.bodyStrong, matchLineBox: false)
                .foregroundStyle(isPlaceholder ? KzColor.inkMuted : KzColor.ink)
                .lineLimit(1)
            Spacer(minLength: 0)
            KzIcon(.chevronsV, size: 14)
                .foregroundStyle(isEnabled ? KzColor.inkMuted : KzDisabled.foreground)
        }
        .padding(.leading, 10)
        .padding(.trailing, 6)
        .frame(minWidth: 140)
        .frame(height: KzSize.control)
        .overlay {
            if isOpen { RoundedRectangle(cornerRadius: KzRadius.md).strokeBorder(KzColor.line) }
        }
    }
}

/// The select well as a button style: `surface-sunken`, a `line` border on hover, `radius-md`, the
/// shared focus ring and disabled treatment. Pair it with `KzSelectLabel` in a `KzMenuButton`.
public struct KzSelectStyle: ButtonStyle {
    /// Creates the style.
    public init() {}

    public func makeBody(configuration: Configuration) -> some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.md)
        KzInteractive(shape, isPressed: configuration.isPressed, stateLayer: false) { state in
            configuration.label
                .foregroundStyle(state.isEnabled ? KzColor.ink : KzDisabled.foreground)
                .background(KzColor.surfaceSunken, in: shape)
                .overlay(shape.strokeBorder(state.isHovered && state.isEnabled ? KzColor.line : .clear))
        }
    }
}
