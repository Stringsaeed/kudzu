// A sidebar item's content (`NavItem` in `sidebar.tsx`): icon, label, trailing marks, count and
// shortcut. Selection and keyboard navigation are the native sidebar `List`'s. The row draws the
// highlight itself, the web's `bg-sidebar-accent shadow-selected` pill, not the system accent color.

import KudzuDesign
import SwiftUI

struct SidebarRow<Trailing: View>: View {
    var icon: Image
    var label: String
    var count: Int?
    var shortcut: String?
    /// The current route (`aria-current="page"`): the pill and an iris icon.
    var selected = false
    /// Turned-off groups (`muted && "text-muted-foreground/70"`).
    var muted = false
    /// A drag is over this row (`over || dropActive`).
    var dropTarget = false
    @ViewBuilder var trailing: Trailing
    @State private var hovered = false

    var body: some View {
        HStack(spacing: 8) {
            icon.icon(16).foregroundStyle(.token(selected ? .iris : .mutedForeground))
            Text(label)
                .font(.system(size: 13))
                .lineLimit(1)
                .truncationMode(.tail)
                .foregroundStyle(.token(muted ? .mutedForeground : .sidebarForeground, alpha: muted ? 0.7 : selected || hovered ? 1 : 0.8))
                .frame(maxWidth: .infinity, alignment: .leading)
            trailing
            metadata
        }
        .padding(.horizontal, 2)
        .frame(height: 24)
        .background {
            let shape = RoundedRectangle(cornerRadius: Radius.md)
            ZStack {
                if dropTarget {
                    shape.fill(.token(.irisSoft))
                        .overlay(shape.strokeBorder(.token(.iris, alpha: 0.4)))
                } else if selected {
                    Color.clear.depth(.selected, fill: Color.kz.sidebarAccent, in: shape)
                } else if hovered {
                    shape.fill(.token(.sidebarAccent))
                }
            }
            // The native row is 28 pt (`h-7`) with 6 pt of content inset on each side.
            .padding(.horizontal, -6)
            .padding(.vertical, -2)
        }
        .scaleEffect(dropTarget ? 1.02 : 1)
        .animation(.kz.springSoft(duration: Motion.moderate), value: dropTarget)
        .animation(.kz.colors, value: selected)
        .animation(.kz.colors, value: hovered)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
        .kzOwnSelection()
    }

    private var metadata: some View {
        HStack(spacing: 8) {
            Group {
                if let count, count > 0 {
                    Text("\(count)")
                        .font(.system(size: 10.5, design: .monospaced))
                        .foregroundStyle(.token(.faint))
                        .tick(count)
                } else { Color.clear.accessibilityHidden(true) }
            }
            .frame(width: 26, height: 18, alignment: .trailing)
            Group {
                if let shortcut { Shortcut(shortcut) }
                else { Color.clear.accessibilityHidden(true) }
            }
            .frame(width: 38, height: 18, alignment: .trailing)
        }
    }
}

extension SidebarRow where Trailing == EmptyView {
    init(icon: Image, label: String, count: Int? = nil, shortcut: String? = nil, selected: Bool = false, muted: Bool = false, dropTarget: Bool = false) {
        self.init(icon: icon, label: label, count: count, shortcut: shortcut, selected: selected, muted: muted, dropTarget: dropTarget) { EmptyView() }
    }
}

/// A section heading's trailing "Paused" badge (`bg-warning/15 text-warning`).
struct PausedBadge: View {
    var body: some View {
        Text("Paused")
            .font(.system(size: 10, weight: .medium))
            .foregroundStyle(.token(.warning))
            .padding(.horizontal, 6)
            .background(.token(.warning, alpha: 0.15), in: Capsule())
    }
}
