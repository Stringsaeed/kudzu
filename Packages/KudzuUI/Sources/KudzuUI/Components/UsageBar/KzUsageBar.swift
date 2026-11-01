import SwiftUI

/// A ranked usage bar for "Most used": a trigger chip, a thin `vine-ink` bar on a `surface-sunken`
/// track, and the count.
public struct KzUsageBar: View {
    private let label: String
    private let value: Int
    private let maximum: Int

    /// Creates a bar.
    /// - Parameters:
    ///   - label: The trigger, shown as a `KzTriggerChip` in a 96pt column.
    ///   - value: This item's count.
    ///   - maximum: The top value in the set; the bar is `value / maximum` of the track, at least 2%.
    public init(_ label: String, value: Int, maximum: Int) {
        self.label = label
        self.value = value
        self.maximum = maximum
    }

    /// The filled share of the track: `value / maximum`, clamped to 2%…100% (0 when `maximum` is 0).
    static func fraction(value: Int, maximum: Int) -> Double {
        guard maximum > 0 else { return 0 }
        return min(1, max(0.02, Double(value) / Double(maximum)))
    }

    public var body: some View {
        HStack(spacing: KzSpace.s3) {
            KzTriggerChip(label)
                .frame(width: 96, alignment: .leading)
                .clipped()
            GeometryReader { proxy in
                ZStack(alignment: .leading) {
                    Capsule().fill(KzColor.surfaceSunken)
                    Capsule().fill(KzColor.vineInk)
                        .frame(width: proxy.size.width * Self.fraction(value: value, maximum: maximum))
                }
            }
            .frame(height: 8)
            Text("\(value)")
                .kzText(.adhoc(size: 12, lineHeight: 16, weight: 500, tabular: true))
                .foregroundStyle(KzColor.inkMuted)
                .frame(width: 32, alignment: .trailing)
        }
        .frame(minHeight: 28)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(label)
        .accessibilityValue("\(value) of \(maximum)")
    }
}
