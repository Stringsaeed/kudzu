// `Segmented` (`segmented.tsx`): a radio group drawn as a segmented control. A recessed well
// (`bg-foreground/[0.04] shadow-well`) holds the options, and the selected one sits on the raised
// `SelectionPill` (`bg-surface-raised shadow-soft sheen`) that slides between options. The system
// segmented control would paint the selection in the accent color, which the web never does.
// ←/→ (and ↑/↓) move the selection, as the web's radio group does.

import SwiftUI

/// One choice in a `Segmented` control.
public struct SegmentOption<Value: Hashable>: Identifiable {
    /// The value the option selects.
    public var value: Value
    /// The user-visible label.
    public var label: String
    /// The tooltip (`title={description}`).
    public var help: String?
    /// Stable identity for this value.
    public var id: Value { value }

    /// Creates SegmentOption with the supplied content and styling.
    public init(_ label: String, value: Value, help: String? = nil) {
        self.label = label
        self.value = value
        self.help = help
    }
}

/// A segmented radio group in Kudzu's style.
public struct Segmented<Value: Hashable>: View {
    /// `md` is `h-7 text-xs`; `sm` is `h-6 text-[11px]`.
    public enum Size: Sendable { case md, sm }

    var label: String
    @Binding var selection: Value
    var options: [SegmentOption<Value>]
    var size: Size
    @Namespace private var pill
    @Environment(\.colorScheme) private var scheme

    /// Creates Segmented with the supplied content and styling.
    public init(_ label: String, selection: Binding<Value>, size: Size = .md, options: [SegmentOption<Value>]) {
        self.label = label
        _selection = selection
        self.size = size
        self.options = options
    }

    /// The composed native view.
    public var body: some View {
        let t = KZTheme(scheme)
        HStack(spacing: 0) {
            ForEach(options) { option in
                SegmentButton(option: option, selected: option.value == selection, size: size, namespace: pill) {
                    selection = option.value
                }
            }
        }
        .padding(2)
        .depth(.well, fill: t[.foreground, alpha: 0.04], in: RoundedRectangle(cornerRadius: Radius.lg))
        .fixedSize()
        .focusable(interactions: .activate)
        .onKeyPress(keys: [.leftArrow, .rightArrow, .upArrow, .downArrow]) { press in
            let step = press.key == .rightArrow || press.key == .downArrow ? 1 : -1
            let index = options.firstIndex { $0.value == selection } ?? 0
            selection = options[(index + step + options.count) % options.count].value
            return .handled
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(label)
    }
}

private struct SegmentButton<Value: Hashable>: View {
    var option: SegmentOption<Value>
    var selected: Bool
    var size: Segmented<Value>.Size
    var namespace: Namespace.ID
    var action: () -> Void
    @State private var hovered = false
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let t = KZTheme(scheme)
        Button(action: action) {
            Text(option.label)
                .font(.system(size: size == .md ? 12 : 11, weight: .medium))
                .lineLimit(1)
                .foregroundStyle(selected || hovered ? t[.foreground] : t[.mutedForeground])
                .padding(.horizontal, 10)
                .frame(height: size == .md ? 28 : 24)
                .contentShape(Rectangle())
        }
        .buttonStyle(.press)
        .selectionPill(selected, in: namespace)
        .onHover { hovered = $0 }
        .animation(.kz.colors, value: hovered)
        .animation(.kz.colors, value: selected)
        .help(option.help ?? "")
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}
