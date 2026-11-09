// `AppIcon` (`app-icon.tsx`), `Wordmark` (`wordmark.tsx`), `Breadcrumbs` (`breadcrumbs.tsx`) and
// `InsightBar` (the ranked bars in `insights.tsx`).

import AppKit
import SwiftUI

/// An app's icon from its bundle (`NSWorkspace`), falling back to a monogram tile:
/// `rounded-[5px] bg-foreground/[0.08] font-semibold text-muted-foreground`, text at half the size.
public struct AppIcon: View {
    var path: String?
    var name: String
    var size: CGFloat
    @Environment(\.colorScheme) private var scheme

    /// Creates AppIcon with the supplied content and styling.
    public init(path: String?, name: String, size: CGFloat = 18) {
        self.path = path
        self.name = name
        self.size = size
    }

    /// The composed native view.
    public var body: some View {
        if let path, FileManager.default.fileExists(atPath: path) {
            Image(nsImage: NSWorkspace.shared.icon(forFile: path))
                .resizable()
                .interpolation(.high)
                .frame(width: size, height: size)
                .accessibilityHidden(true)
        } else {
            let t = KZTheme(scheme)
            Text(name.prefix(1).uppercased())
                .font(.system(size: size * 0.5, weight: .semibold))
                .foregroundStyle(t[.mutedForeground])
                .frame(width: size, height: size)
                .background(t[.foreground, alpha: 0.08], in: RoundedRectangle(cornerRadius: Radius.key))
                .accessibilityHidden(true)
        }
    }
}

// MARK: Wordmark

/// The Echo wordmark (`design/wordmark/*.svg`): stepped, fading outlines behind block letters,
/// drawn in the current foreground style like the web's `bg-current` mask.
public struct Wordmark: View {
    /// The full or small Echo wordmark artwork.
    public enum Variant: Sendable { case full, small }
    var variant: Variant

    /// The brand blue the wordmark is shown on in brand material (white on it).
    public static let brandBlue = Color(.sRGB, red: 0x14 / 255, green: 0x47 / 255, blue: 0xE6 / 255) // #1447E6, the app icon's Echo K (design/app-icon/layers/02-echo-k-printed.svg)

    /// Creates Variant with the supplied content and styling.
    public init(_ variant: Variant = .full) { self.variant = variant }

    /// The composed native view.
    public var body: some View {
        let art = variant == .full ? WordmarkArt.full : WordmarkArt.small
        Rectangle()
            .fill(.foreground)
            .mask {
                Canvas { ctx, size in
                    let scale = min(size.width / art.viewBox.width, size.height / art.viewBox.height)
                    let dx = (size.width - art.viewBox.width * scale) / 2 - art.viewBox.minX * scale
                    let dy = (size.height - art.viewBox.height * scale) / 2 - art.viewBox.minY * scale
                    for layer in art.layers {
                        let path = layer.path.applying(CGAffineTransform(a: scale, b: 0, c: 0, d: scale, tx: dx, ty: dy))
                        ctx.fill(path, with: .color(.white.opacity(layer.opacity)), style: FillStyle(eoFill: true))
                    }
                }
            }
            .aspectRatio(art.viewBox.width / art.viewBox.height, contentMode: .fit)
            .accessibilityElement()
            .accessibilityLabel("Kudzu")
            .accessibilityAddTraits(.isImage)
    }
}

// MARK: Breadcrumbs

/// A group name and optional navigation action.
public struct BreadcrumbEntry: Identifiable {
    /// Stable identity for this value.
    public var id: Int
    /// The user-visible label.
    public var label: String
    /// The `onSelect` design-system value.
    public var onSelect: (@MainActor () -> Void)?
    /// Creates BreadcrumbEntry with the supplied content and styling.
    public init(id: Int, label: String, onSelect: (@MainActor () -> Void)? = nil) {
        self.id = id
        self.label = label
        self.onSelect = onSelect
    }
}

/// Group breadcrumbs: 13 pt muted, chevron separators (`text-faint size-3.5`), the last item as the
/// current page (medium, foreground). More than four items collapse the middle into a `…` menu.
public struct Breadcrumbs: View {
    var items: [BreadcrumbEntry]
    @Environment(\.colorScheme) private var scheme

    /// Creates Breadcrumbs with the supplied content and styling.
    public init(_ items: [BreadcrumbEntry]) { self.items = items }

    /// The composed native view.
    public var body: some View {
        let t = KZTheme(scheme)
        let collapsed = items.count > 4
        let hidden = collapsed ? Array(items[1..<(items.count - 2)]) : []
        let visible = collapsed ? [items[0]] + items.suffix(2) : items
        HStack(spacing: 8) {
            ForEach(Array(visible.enumerated()), id: \.element.id) { index, item in
                if index > 0 { separator(t) }
                if collapsed && index == 1 {
                    Menu {
                        ForEach(hidden) { parent in Button(parent.label) { parent.onSelect?() } }
                    } label: {
                        Icon.moreHorizontal.icon(16)
                    }
                    .kzMenuTrigger(.ghost, size: .iconXs)
                    .accessibilityLabel("Show parent groups")
                    separator(t)
                }
                if index == visible.count - 1 {
                    Text(item.label).font(.system(size: 13, weight: .medium)).foregroundStyle(t[.foreground]).lineLimit(1).help(item.label)
                        .frame(maxWidth: .infinity, alignment: .leading)
                } else {
                    CappedWidth(max: 160) { BreadcrumbLink(item: item) } // `max-w-40`
                }
            }
        }
        .font(.system(size: 13))
        .foregroundStyle(t[.mutedForeground])
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Breadcrumb")
    }

    private func separator(_ t: KZTheme) -> some View {
        Icon.chevronRight.icon(14).foregroundStyle(t[.faint]).accessibilityHidden(true)
    }
}

/// Sizes its one child to its natural width, at most `max`, truncating beyond. `.frame(maxWidth:)`
/// would instead take the whole `max` whenever the row has room, leaving a gap after short labels.
private struct CappedWidth: Layout {
    var max: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        guard let child = subviews.first else { return .zero }
        let natural = child.sizeThatFits(.unspecified).width
        let width = min(natural, max, proposal.width ?? .infinity)
        return child.sizeThatFits(ProposedViewSize(width: width, height: proposal.height))
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        subviews.first?.place(at: bounds.origin, proposal: ProposedViewSize(bounds.size))
    }
}

private struct BreadcrumbLink: View {
    var item: BreadcrumbEntry
    @State private var hovered = false
    @Environment(\.colorScheme) private var scheme
    var body: some View {
        Button(item.label) { item.onSelect?() }
            .buttonStyle(.plain)
            .lineLimit(1)
            .foregroundStyle(hovered ? KZTheme(scheme)[.foreground] : KZTheme(scheme)[.mutedForeground])
            .onHover { hovered = $0 }
            .animation(.kz.colors, value: hovered)
            .help(item.label)
    }
}

// MARK: InsightBar

/// A ranked usage bar: `h-1 rounded-full bg-foreground/[0.05]` track, `bg-iris/80` fill at
/// `max(3%, value)`, growing from the leading edge (`animate-bar`, 60 ms + 40 ms × rank delay).
public struct InsightBar: View {
    var fraction: Double
    var rank: Int
    @Environment(\.colorScheme) private var scheme

    /// Creates InsightBar with the supplied content and styling.
    public init(fraction: Double, rank: Int = 0) {
        self.fraction = fraction
        self.rank = rank
    }

    /// The composed native view.
    public var body: some View {
        let t = KZTheme(scheme)
        GeometryReader { proxy in
            Capsule()
                .fill(t[.iris, alpha: 0.8])
                .frame(width: proxy.size.width * max(0.03, min(1, fraction)))
                .barGrow(delay: 0.06 + 0.04 * Double(rank))
                .animation(.kz.ease(duration: 0.5), value: fraction)
        }
        .frame(height: 4)
        .background(t[.foreground, alpha: 0.05], in: Capsule())
        .clipShape(Capsule())
    }
}
