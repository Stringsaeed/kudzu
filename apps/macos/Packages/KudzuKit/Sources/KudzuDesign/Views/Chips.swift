// Small inline views with no native counterpart: `Badge` (`badge.tsx`), `TriggerChip`
// (`trigger-chip.tsx`) and `Skeleton`.

import SwiftUI

/// Colour treatments for compact badges.
public enum BadgeVariant: String, CaseIterable, Sendable {
    case `default`, secondary, destructive, outline, ghost, link
}

/// `h-5 rounded-4xl px-2 py-0.5 text-xs font-medium gap-1`, 12 pt icons.
public struct Badge<Content: View>: View {
    var variant: BadgeVariant
    var content: Content
    @Environment(\.colorScheme) private var scheme
    @State private var hovered = false

    /// Creates Badge with the supplied content and styling.
    public init(_ variant: BadgeVariant = .default, @ViewBuilder content: () -> Content) {
        self.variant = variant
        self.content = content()
    }

    /// The composed native view.
    public var body: some View {
        let t = KZTheme(scheme)
        let shape = Capsule()
        content
            .font(.system(size: 12, weight: .medium))
            .lineLimit(1)
            .iconSize(12)
            .foregroundStyle(text(t))
            .underline(variant == .link && hovered)
            .padding(.horizontal, 8)
            .frame(height: 20)
            .background(fill(t), in: shape)
            .overlay { if variant == .outline { shape.strokeBorder(t[.border], lineWidth: 1) } }
            .onHover { hovered = $0 }
    }

    private func text(_ t: KZTheme) -> Color {
        switch variant {
        case .default: t[.primaryForeground]
        case .secondary: t[.secondaryForeground]
        case .destructive: t[.destructive]
        case .outline: t[.foreground]
        case .ghost: hovered ? t[.mutedForeground] : t[.foreground]
        case .link: t[.primary]
        }
    }

    private func fill(_ t: KZTheme) -> Color {
        switch variant {
        case .default: t[.primary]
        case .secondary: t[.secondary]
        case .destructive: t[.destructive, alpha: t.dark ? 0.2 : 0.1]
        case .ghost: hovered ? (t.dark ? t[.muted, alpha: 0.5] : t[.muted]) : .clear
        case .outline, .link: .clear
        }
    }
}

extension Badge where Content == Text {
    /// Creates Badge with the supplied content and styling.
    public init(_ title: String, variant: BadgeVariant = .default) {
        self.init(variant) { Text(title) }
    }
}

/// A trigger rendered as a mono token: `h-5 rounded-[5px] px-1.5`, 11.5 pt Paper Mono,
/// `tracking-tight`, spaces shown as `␣`; iris-soft on iris-foreground, or muted.
public struct TriggerChip: View {
    var trigger: String
    var muted: Bool
    @Environment(\.colorScheme) private var scheme

    /// Creates TriggerChip with the supplied content and styling.
    public init(_ trigger: String, muted: Bool = false) {
        self.trigger = trigger
        self.muted = muted
    }

    /// The composed native view.
    public var body: some View {
        let t = KZTheme(scheme)
        Text(trigger.replacingOccurrences(of: " ", with: "␣"))
            .font(KZFonts.mono(size: 11.5))
            .tracking(Tracking.tight * 11.5)
            .lineLimit(1)
            .truncationMode(.tail)
            .foregroundStyle(muted ? t[.mutedForeground] : t[.irisForeground])
            .padding(.horizontal, 6)
            .frame(height: 20)
            .background(muted ? t[.foreground, alpha: 0.05] : t[.irisSoft], in: RoundedRectangle(cornerRadius: Radius.key))
    }
}

/// `Skeleton`: a muted rounded block with the loading shimmer
/// (`bg-[linear-gradient(90deg,transparent,color-mix(…foreground 8%…),transparent)] animate-shimmer`).
public struct Skeleton: View {
    var radius: CGFloat
    var strength: Double

    /// `strength`: the highlight's foreground mix (0.08 in the palette, 0.10 in clip previews).
    public init(radius: CGFloat = 4, strength: Double = 0.08) {
        self.radius = radius
        self.strength = strength
    }

    /// The composed native view.
    public var body: some View {
        Color.clear.shimmer(Color.kz.mix(.foreground, alpha: strength), in: RoundedRectangle(cornerRadius: radius))
    }
}
