// The `Item` family (`item.tsx`): row content for `List`s and stacks. `Item` is the row container
// (`rounded-lg border gap-2.5 px-3 py-2.5 text-sm`); inside a native `List` use it with
// `.kzListRow()` and let the list draw selection.

import SwiftUI

/// Surface treatments for list-row content.
public enum ItemVariant: Sendable { case `default`, outline, muted }
/// Spacing variants for list-row content.
public enum ItemSize: Sendable { case `default`, sm, xs }
/// Leading icon and image treatments in an item.
public enum ItemMediaVariant: Sendable { case `default`, icon, image }

extension EnvironmentValues {
    @Entry var kzItemSize: ItemSize = .default
}

/// Composes media, content and actions into a native list row.
public struct Item<Content: View>: View {
    var variant: ItemVariant
    var size: ItemSize
    var content: Content
    @Environment(\.colorScheme) private var scheme

    /// Creates Item with the supplied content and styling.
    public init(_ variant: ItemVariant = .default, size: ItemSize = .default, @ViewBuilder content: () -> Content) {
        self.variant = variant
        self.size = size
        self.content = content()
    }

    /// The composed native view.
    public var body: some View {
        let t = KZTheme(scheme)
        let shape = RoundedRectangle(cornerRadius: Radius.lg)
        HStack(alignment: .center, spacing: size == .xs ? 8 : 10) { content }
            .font(.system(size: 14))
            .padding(.horizontal, size == .xs ? 10 : 12)
            .padding(.vertical, size == .xs ? 8 : 10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(variant == .muted ? t[.muted, alpha: 0.5] : .clear, in: shape)
            .overlay { if variant == .outline { shape.strokeBorder(t[.border], lineWidth: 1) } }
            .environment(\.kzItemSize, size)
    }
}

/// Leading media in an item row.
public struct ItemMedia<Content: View>: View {
    var variant: ItemMediaVariant
    var content: Content
    @Environment(\.kzItemSize) private var size

    /// Creates ItemMedia with the supplied content and styling.
    public init(_ variant: ItemMediaVariant = .default, @ViewBuilder content: () -> Content) {
        self.variant = variant
        self.content = content()
    }

    /// The composed native view.
    public var body: some View {
        switch variant {
        case .default: content
        case .icon: content.iconSize(16)
        case .image:
            let d: CGFloat = size == .xs ? 24 : (size == .sm ? 32 : 40)
            content.frame(width: d, height: d).clipShape(RoundedRectangle(cornerRadius: Radius.sm))
        }
    }
}

/// The expanding text column in an item row.
public struct ItemContent<Content: View>: View {
    var content: Content
    @Environment(\.kzItemSize) private var size
    /// Creates ItemContent with the supplied content and styling.
    public init(@ViewBuilder content: () -> Content) { self.content = content() }
    /// The composed native view.
    public var body: some View {
        VStack(alignment: .leading, spacing: size == .xs ? 0 : 4) { content }
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// `line-clamp-1 text-sm leading-snug font-medium`.
public struct ItemTitle: View {
    var text: String
    /// Creates ItemTitle with the supplied content and styling.
    public init(_ text: String) { self.text = text }
    /// The composed native view.
    public var body: some View {
        Text(text).font(.system(size: 14, weight: .medium)).lineLimit(1)
    }
}

/// `line-clamp-2 text-sm leading-normal text-muted-foreground` (xs: `text-xs`).
public struct ItemDescription: View {
    var text: String
    @Environment(\.kzItemSize) private var size
    /// Creates ItemDescription with the supplied content and styling.
    public init(_ text: String) { self.text = text }
    /// The composed native view.
    public var body: some View {
        Text(text).font(.system(size: size == .xs ? 12 : 14)).lineLimit(2).mutedForeground()
    }
}

/// Trailing actions in an item row.
public struct ItemActions<Content: View>: View {
    var content: Content
    /// Creates ItemActions with the supplied content and styling.
    public init(@ViewBuilder content: () -> Content) { self.content = content() }
    /// The composed native view.
    public var body: some View { HStack(spacing: 8) { content } }
}
