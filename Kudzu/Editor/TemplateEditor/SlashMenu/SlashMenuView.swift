import KudzuCore
import KudzuUI
import SwiftUI

/// Observable state behind the `/` menu.
@MainActor @Observable
final class SlashMenuModel {
    var items: [TemplateInsertItem] = []
    var query = ""
    var selection: TemplateInsertItem?

    /// The menu as `KzMenuSurface` draws it: a titled list of the matching tags. Picking is handled by the panel,
    /// so the items carry no action.
    var menu: KzMenu {
        KzMenu(title: query.isEmpty ? "Insert" : "Insert “\(query)”", elements: items.map {
            .item(KzMenuItem($0.title, icon: ChipStyle.icon(for: $0), id: $0.id) {})
        })
    }
}

/// The Kudzu menu surface, padded by the shadow's reach because the panel is larger than the glass.
///
/// `KzMenuSurface` is the design system's menu, so it looks the same as every other Kudzu menu. It is not the live
/// `KzMenu` session because that takes every key while it is open, and here the keys must keep reaching the text.
struct SlashMenuView: View {
    let model: SlashMenuModel

    var body: some View {
        KzMenuSurface(model.menu, highlightedID: model.selection?.id, blending: .behindWindow)
            .padding(EdgeInsets(top: SlashMenuMetrics.margins.top, leading: SlashMenuMetrics.margins.left,
                                bottom: SlashMenuMetrics.margins.bottom, trailing: SlashMenuMetrics.margins.right))
            .fixedSize()
    }
}

/// The geometry of `KzMenuSurface`, needed to turn a pointer position into a row. These are the menu CSS metrics
/// (`.kz-menu` padding 4, `.kz-menu-title` 24, `.kz-menu-item` 28); KudzuUI's own `KzMenuLayout` is internal.
enum SlashMenuMetrics {
    static let padding: CGFloat = 4
    static let titleHeight: CGFloat = 24
    static let itemHeight: CGFloat = 28

    /// Room the `shadow-float` drop needs around the glass: the furthest any outer layer reaches on each side.
    static let margins: NSEdgeInsets = {
        let shadow = KzShadow.float
        var top: CGFloat = 0, bottom: CGFloat = 0, side: CGFloat = 0
        for layer in shadow.lightLayers + shadow.darkLayers where !layer.inset {
            let reach = layer.blur + layer.spread
            top = max(top, reach - layer.y)
            bottom = max(bottom, reach + layer.y)
            side = max(side, reach + abs(layer.x))
        }
        return NSEdgeInsets(top: ceil(top) + 4, left: ceil(side) + 4, bottom: ceil(bottom) + 4, right: ceil(side) + 4)
    }()

    /// The item row under `y` (points below the glass's top edge), or nil over the title, the padding or past the end.
    static func row(atY y: CGFloat, itemCount: Int) -> Int? {
        let offset = y - padding - titleHeight
        guard offset >= 0 else { return nil }
        let row = Int(offset / itemHeight)
        return row < itemCount ? row : nil
    }
}
