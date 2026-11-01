import SwiftUI

/// A bounded integer setting with native decrement/increment buttons and adjustable accessibility.
public struct KzStepper: View {
    @Binding private var value: Int
    private let step: Int
    private let minimum: Int
    private let maximum: Int
    private let unit: String
    private let label: String
    @Environment(\.isEnabled) private var isEnabled
    /// Creates a stepper with a positive step, inclusive bounds, and a lowercase unit.
    public init(_ label: String, value: Binding<Int>, step: Int = 1, min: Int = .min, max: Int = .max, unit: String = "") {
        precondition(step > 0 && min <= max)
        self.label = label; _value = value; self.step = step; minimum = min; maximum = max; self.unit = unit.lowercased()
    }
    public var body: some View {
        HStack(spacing: 0) {
            Button { adjust(-1) } label: { KzIcon(.minus, size: 12) }
                .buttonStyle(StepStyle()).disabled(value <= minimum).accessibilityLabel("Decrease \(label)")
            HStack(spacing: 4) {
                Text(value, format: .number.grouping(.never)).kzTextStyle(.bodyStrong, matchLineBox: false).monospacedDigit()
                if !unit.isEmpty { Text(unit).kzTextStyle(.body, matchLineBox: false).foregroundStyle(isEnabled ? KzColor.inkMuted : KzDisabled.foreground) }
            }.foregroundStyle(isEnabled ? KzColor.ink : KzDisabled.foreground).frame(minWidth: 64)
            Button { adjust(1) } label: { KzIcon(.plus, size: 12) }
                .buttonStyle(StepStyle()).disabled(value >= maximum).accessibilityLabel("Increase \(label)")
        }
        .frame(height: 30).background(KzColor.surfaceSunken, in: RoundedRectangle(cornerRadius: KzRadius.md))
        .onKeyPress(.upArrow) { adjust(1); return .handled }
        .onKeyPress(.rightArrow) { adjust(1); return .handled }
        .onKeyPress(.downArrow) { adjust(-1); return .handled }
        .onKeyPress(.leftArrow) { adjust(-1); return .handled }
        .accessibilityElement(children: .ignore).accessibilityLabel(label)
        .accessibilityValue("\(value) \(unit)")
        .accessibilityAdjustableAction { direction in
            switch direction { case .increment: adjust(1); case .decrement: adjust(-1); @unknown default: break }
        }
    }
    private func adjust(_ direction: Int) {
        guard isEnabled else { return }
        value = Self.adjusted(value, direction: direction, step: step, min: minimum, max: maximum)
    }
    static func adjusted(_ value: Int, direction: Int, step: Int, min: Int, max: Int) -> Int {
        let (next, overflow) = value.addingReportingOverflow(direction > 0 ? step : -step)
        return Swift.max(min, Swift.min(max, overflow ? (direction > 0 ? Int.max : Int.min) : next))
    }
    private struct StepStyle: ButtonStyle {
        func makeBody(configuration: Configuration) -> some View {
            let shape = RoundedRectangle(cornerRadius: KzRadius.sm)
            KzInteractive(shape, isPressed: configuration.isPressed, stateLayer: false) { state in
                configuration.label
                    .foregroundStyle(!state.isEnabled ? KzDisabled.foreground : state.isHovered ? KzColor.ink : KzColor.inkMuted)
                    .frame(width: 28, height: 28)
                    .background(state.isPressed ? KzColor.surfacePressed : state.isHovered ? KzColor.surfaceRaised : .clear, in: shape)
                    .background { if state.isHovered { shape.fill(.clear).kzShadow(.control, in: shape) } }
            }.padding(1)
        }
    }
}
