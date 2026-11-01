import SwiftUI

/// A neutral two-to-four-option segmented selector with native Picker accessibility.
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
    /// Creates a selector; provide two through four distinct values.
    public init(_ label: String, selection: Binding<Value>, options: [Option]) {
        precondition((2...4).contains(options.count) && Set(options.map(\.value)).count == options.count)
        self.label = label; _selection = selection; self.options = options
    }
    public var body: some View {
        HStack(spacing: 2) {
            ForEach(options) { option in
                Button { selection = option.value } label: {
                    Text(option.label).kzTextStyle(.callout, matchLineBox: false).fontWeight(.medium)
                }
                .buttonStyle(SegmentStyle(selected: selection == option.value))
                .onKeyPress(.leftArrow) { move(-1); return .handled }
                .onKeyPress(.rightArrow) { move(1); return .handled }
            }
        }
        .padding(2).background(KzColor.surfaceSunken, in: RoundedRectangle(cornerRadius: KzRadius.md))
        .accessibilityRepresentation {
            Picker(label, selection: $selection) {
                ForEach(options) { Text($0.label).tag($0.value) }
            }.pickerStyle(.segmented)
        }
    }
    private func move(_ delta: Int) {
        guard let index = options.firstIndex(where: { $0.value == selection }) else { return }
        selection = options[Swift.max(0, Swift.min(options.count - 1, index + delta))].value
    }
    private struct SegmentStyle: ButtonStyle {
        let selected: Bool
        func makeBody(configuration: Configuration) -> some View {
            let shape = RoundedRectangle(cornerRadius: KzRadius.sm)
            KzInteractive(shape, isPressed: configuration.isPressed, stateLayer: false) { state in
                configuration.label
                    .foregroundStyle(!state.isEnabled ? KzDisabled.foreground : selected || state.isHovered ? KzColor.ink : KzColor.inkMuted)
                    .padding(.horizontal, 12).frame(height: 26)
                    .background(selected ? KzColor.surfaceRaised : .clear, in: shape)
                    .overlay { if selected { shape.stroke(KzColor.line, lineWidth: 1) } }
                    .background { if selected { shape.fill(.clear).kzShadow(.control, in: shape) } }
            }
        }
    }
}
