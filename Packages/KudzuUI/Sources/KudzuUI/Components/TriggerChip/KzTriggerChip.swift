import SwiftUI

/// The monospace chip that shows a trigger, Kudzu's signature element. Shows the trigger exactly as typed.
///
/// A leading symbol (`:` or `;`) is set in `vine-ink` so the prefix reads as syntax. `onRemove`
/// adds a remove button; use it only inside `KzTriggerInput`.
public struct KzTriggerChip: View {
    private let trigger: String
    private let onRemove: (() -> Void)?

    /// Creates a chip for `trigger`, removable when `onRemove` is given.
    public init(_ trigger: String, onRemove: (() -> Void)? = nil) {
        self.trigger = trigger
        self.onRemove = onRemove
    }

    /// The leading symbol shown in `vine-ink`, if the trigger starts with one.
    static func split(_ trigger: String) -> (lead: String, body: String) {
        guard let first = trigger.first, !first.isLetter, !first.isNumber else { return ("", trigger) }
        return (String(first), String(trigger.dropFirst()))
    }

    public var body: some View {
        let parts = Self.split(trigger)
        HStack(spacing: 0) {
            (Text(parts.lead).foregroundStyle(KzColor.vineInk) + Text(parts.body).foregroundStyle(KzColor.ink))
                .kzTextStyle(.trigger, matchLineBox: false)
                .lineLimit(1)
                .fixedSize()
            if let onRemove {
                Button(action: onRemove) { KzIcon(.x, size: 10) }
                    .buttonStyle(RemoveStyle())
                    .padding(.leading, KzSpace.s1)
                    .accessibilityLabel("Remove \(trigger)")
            }
        }
        .padding(.leading, 6)
        .padding(.trailing, onRemove == nil ? 6 : 2)
        .frame(height: onRemove == nil ? 20 : 22)
        .background(KzColor.chip, in: RoundedRectangle(cornerRadius: KzRadius.sm))
        .accessibilityElement(children: .contain)
        .accessibilityLabel(trigger)
    }

    private struct RemoveStyle: ButtonStyle {
        func makeBody(configuration: Configuration) -> some View {
            let shape = RoundedRectangle(cornerRadius: KzRadius.xs)
            KzInteractive(shape, isPressed: configuration.isPressed, stateLayer: false) { state in
                configuration.label
                    .foregroundStyle(state.isHovered ? KzColor.ink : KzColor.inkMuted)
                    .frame(width: 16, height: 16)
                    .background(state.isHovered || state.isPressed ? KzColor.surfacePressed : .clear, in: shape)
            }
        }
    }
}
