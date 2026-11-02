import SwiftUI

/// A short status label: backup kind, "Coming soon", a favorites marker. The word carries the meaning.
public struct KzBadge: View {
    /// Each tone is its own ink on its own soft fill.
    public enum Tone: Sendable {
        /// Default: `ink-muted` on `chip`.
        case neutral
        /// Active or on: `vine-ink` on `vine-soft`.
        case vine
        /// Favorites only: `bloom` on `bloom-soft`.
        case bloom
        /// Caution: `warning` on `warning-soft`.
        case warning
        /// Errors: `danger` on `danger-soft`.
        case danger
    }

    private let text: String
    private let tone: Tone

    /// Creates a badge with one or two words.
    public init(_ text: String, tone: Tone = .neutral) {
        self.text = text
        self.tone = tone
    }

    public var body: some View {
        let (ink, fill) = colors
        Text(text)
            .kzTextStyle(.caption, matchLineBox: false)
            .tracking(0)
            .lineLimit(1)
            .foregroundStyle(ink)
            .padding(.horizontal, KzSpace.s2)
            .frame(height: 20)
            .background(fill, in: Capsule())
            .fixedSize()
    }

    private var colors: (Color, Color) {
        switch tone {
        case .neutral: (KzColor.inkMuted, KzColor.chip)
        case .vine: (KzColor.vineInk, KzColor.vineSoft)
        case .bloom: (KzColor.bloom, KzColor.bloomSoft)
        case .warning: (KzColor.warning, KzColor.warningSoft)
        case .danger: (KzColor.danger, KzColor.dangerSoft)
        }
    }
}
