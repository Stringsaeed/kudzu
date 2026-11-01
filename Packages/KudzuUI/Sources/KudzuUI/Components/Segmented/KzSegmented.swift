import SwiftUI

/// A neutral two-to-four-option segmented selector with native Picker accessibility.
///
/// Like `NSSegmentedControl` it is one tab stop with the focus ring around the whole control.
/// ← and → change the selection directly (there is no separate focused segment), so Space has
/// nothing left to press.
public struct KzSegmented<Value: Hashable>: View {
    /// A stable value and its visible, spoken label.
    public struct Option: Identifiable {
        /// The selection value.
        public let value: Value
        /// Sentence-case display label.
        public let label: String
        /// Stable identity for the segment.
        public var id: Value { value }
        /// Creates a labelled selection value.
        public init(_ value: Value, label: String) { self.value = value; self.label = label }
    }
    private let label: String
    private let options: [Option]
    @Binding private var selection: Value
    @Environment(\.isEnabled) private var isEnabled
    /// Creates a selector; provide two through four distinct values.
    public init(_ label: String, selection: Binding<Value>, options: [Option]) {
        precondition((2...4).contains(options.count) && Set(options.map(\.value)).count == options.count)
        self.label = label; _selection = selection; self.options = options
    }
    public var body: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.md)
        HStack(spacing: 2) {
            ForEach(options) { option in
                Segment(label: option.label, isSelected: selection == option.value)
                    .onTapGesture { selection = option.value }
            }
        }
        .padding(2).background(KzColor.surfaceSunken, in: shape)
        // Inside `.focusable`, so the ring reads this control's focus. Like a button, it only takes
        // focus from the keyboard (Tab with Keyboard navigation on), not from a click.
        .kzFocusRing(shape)
        .focusable(isEnabled, interactions: .activate)
        .onKeyPress(.leftArrow) { move(-1); return .handled }
        .onKeyPress(.rightArrow) { move(1); return .handled }
        .onKeyPress(.space) { .handled }
        .accessibilityRepresentation {
            Picker(label, selection: $selection) {
                ForEach(options) { Text($0.label).tag($0.value) }
            }.pickerStyle(.segmented)
        }
    }
    private func move(_ delta: Int) {
        selection = Self.moved(selection, by: delta, in: options.map(\.value))
    }
    /// The value `delta` segments from `value`, stopping at the ends.
    static func moved(_ value: Value, by delta: Int, in values: [Value]) -> Value {
        guard let index = values.firstIndex(of: value) else { return value }
        return values[Swift.max(0, Swift.min(values.count - 1, index + delta))]
    }
    private struct Segment: View {
        let label: String
        let isSelected: Bool
        var body: some View {
            let shape = RoundedRectangle(cornerRadius: KzRadius.sm)
            // The control draws the focus ring, so the segments don't.
            KzInteractive(shape, stateLayer: false, focusRing: nil) { state in
                Text(label).kzTextStyle(.callout, matchLineBox: false).fontWeight(.medium)
                    .foregroundStyle(!state.isEnabled ? KzDisabled.foreground : isSelected || state.isHovered ? KzColor.ink : KzColor.inkMuted)
                    .padding(.horizontal, 12).frame(height: 26)
                    .background(isSelected ? KzColor.surfaceRaised : .clear, in: shape)
                    .overlay { if isSelected { shape.stroke(KzColor.line, lineWidth: 1) } }
                    .background { if isSelected { shape.fill(.clear).kzShadow(.control, in: shape) } }
            }
        }
    }
}
