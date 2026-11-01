import SwiftUI

/// The menu's glass surface as it's drawn in its floating panel, for static specimens in the
/// gallery and snapshots. To show a working menu use `.kzMenu`, `KzMenuButton` or `.kzContextMenu`.
public struct KzMenuSurface: View {
    private let menu: KzMenu
    private let highlightedID: String?
    private let blending: KzGlassBlending

    /// Draws `menu` with the item `highlightedID` highlighted (`vine-soft`, as on hover or keyboard focus).
    public init(_ menu: KzMenu, highlightedID: String? = nil, blending: KzGlassBlending = .withinWindow) {
        self.menu = menu
        self.highlightedID = highlightedID
        self.blending = blending
    }

    public var body: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.lg)
        let highlighted = highlightedID.flatMap { id in menu.elements.firstIndex { $0.item?.id == id } }
        KzMenuRows(menu: menu, highlighted: highlighted)
            .fixedSize()
            .clipShape(shape)
            .kzGlass(.strong, in: shape, blending: blending, shadow: .float)
    }
}

/// The title and rows of a menu, laid out with the CSS metrics `KzMenuLayout` uses for hit-testing.
struct KzMenuRows: View {
    let menu: KzMenu
    let highlighted: Int?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let title = menu.title {
                KzMenuHeading(title)
            }
            ForEach(menu.elements.indices, id: \.self) { index in
                switch menu.elements[index] {
                case .item(let item):
                    KzMenuItemRow(item: item, isHighlighted: highlighted == index && item.isEnabled)
                case .separator:
                    Rectangle()
                        .fill(KzColor.line)
                        .frame(height: 1)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 4)
                case .header(let title):
                    KzMenuHeading(title)
                }
            }
        }
        .padding(KzMenuLayout.padding)
        .frame(minWidth: KzMenuLayout.minWidth, alignment: .leading)
    }
}

/// `.kz-menu-title`: `caption` in `ink-muted`, padded 6 8 4.
private struct KzMenuHeading: View {
    let title: String

    init(_ title: String) { self.title = title }

    var body: some View {
        Text(title)
            .kzTextStyle(.caption)
            .foregroundStyle(KzColor.inkMuted)
            .lineLimit(1)
            .padding(EdgeInsets(top: 6, leading: 8, bottom: 4, trailing: 8))
            .frame(maxWidth: .infinity, minHeight: KzMenuLayout.titleHeight, maxHeight: KzMenuLayout.titleHeight, alignment: .leading)
    }
}

/// `.kz-menu-item`: 28pt, `radius-sm`, a 16pt check/icon column, the label, then the shortcut.
struct KzMenuItemRow: View {
    let item: KzMenuItem
    let isHighlighted: Bool

    private var foreground: Color {
        guard item.isEnabled else { return KzDisabled.foreground }
        return item.role == .danger ? KzColor.danger : KzColor.ink
    }

    private var markColor: Color {
        guard item.isEnabled else { return KzDisabled.foreground }
        if item.role == .danger { return KzColor.danger }
        return KzColor.vineInk
    }

    private var iconColor: Color {
        guard item.isEnabled else { return KzDisabled.foreground }
        if item.role == .danger { return KzColor.danger }
        return isHighlighted ? KzColor.ink : KzColor.inkMuted
    }

    private var secondaryColor: Color {
        item.isEnabled ? KzColor.inkMuted : KzDisabled.foreground
    }

    var body: some View {
        HStack(spacing: KzSpace.s2) {
            // `.kz-menu-check` is always 16pt wide, so labels line up whether or not a row has a mark.
            ZStack {
                if item.isChecked == true {
                    KzIcon(.check, size: 14).foregroundStyle(markColor)
                } else if let icon = item.icon {
                    KzIcon(icon, size: 14).foregroundStyle(iconColor)
                }
            }
            .frame(width: 16, height: 16)

            Text(item.label)
                .kzTextStyle(.body)
                .foregroundStyle(foreground)
                .lineLimit(1)
                .fixedSize()
                .frame(maxWidth: .infinity, alignment: .leading)

            if let shortcut = item.shortcut {
                Text(shortcut.kzGlyphs)
                    .kzTextStyle(.monoCaption)
                    .foregroundStyle(secondaryColor)
                    .fixedSize()
                    // A little more room than the CSS gap, as Mac menus keep shortcuts clear of labels.
                    .padding(.leading, KzSpace.s4)
            }
            if item.hasSubmenu {
                KzIcon(.chevronRight, size: 12).foregroundStyle(secondaryColor)
            }
        }
        .padding(.leading, 6)
        .padding(.trailing, KzSpace.s2)
        .frame(height: KzMenuLayout.itemHeight)
        .background(isHighlighted ? KzColor.vineSoft : .clear, in: RoundedRectangle(cornerRadius: KzRadius.sm))
    }
}
