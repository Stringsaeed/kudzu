import CoreGraphics
import SwiftUI

/// Where a floating layer (menu, popover, panel) goes, in screen coordinates (AppKit: origin at
/// the bottom left, y up). `frame(for:in:)` resolves it against a screen's visible frame, flipping
/// to the other side and clamping the way native menus and popovers do.
public enum KzFloatingPlacement: Equatable, Sendable {
    /// How a layer lines up with its anchor along the edge it opens from.
    public enum Alignment: Equatable, Sendable {
        /// Leading edges aligned (pull-down menus, the form under the caret).
        case leading
        /// Centred on the anchor (popovers).
        case center
        /// Trailing edges aligned.
        case trailing
    }

    /// Beside `anchor` on `edge`, `gap` away, flipping to the opposite edge when there is more room there.
    case edge(Edge, anchor: CGRect, gap: CGFloat = 4, alignment: Alignment = .leading)
    /// Top-left corner at `point` (context menus). Opens up and/or to the left when there's no room.
    case point(CGPoint)
    /// Puts `contentPoint` (measured from the layer's top-left, y down) on `screenPoint`:
    /// pop-up menus put the checked item's label over the field's label this way.
    case aligning(contentPoint: CGPoint, screenPoint: CGPoint)
    /// A submenu: to the right of `parent` (the parent menu's frame) with its top at `top`,
    /// or to the left when it doesn't fit.
    case submenu(parent: CGRect, top: CGFloat, gap: CGFloat = 2)
    /// Centred horizontally on the screen. With `topFraction`, the top edge sits that fraction of
    /// the screen height below the top (the search palette uses 1/6); otherwise centred vertically.
    case centered(topFraction: CGFloat? = nil)

    /// Distance kept from the screen's edges.
    public static let screenMargin: CGFloat = 4

    /// The layer's frame for a layer of `size` on a screen whose visible frame is `screen`.
    public func frame(for size: CGSize, in screen: CGRect) -> CGRect {
        var frame = CGRect(origin: .zero, size: size)
        switch self {
        case .edge(let edge, let anchor, let gap, let alignment):
            switch edge {
            case .bottom, .top:
                frame.origin.x = Self.align(alignment, start: anchor.minX, mid: anchor.midX, end: anchor.maxX, length: size.width)
                let below = anchor.minY - gap - size.height
                let above = anchor.maxY + gap
                let roomBelow = anchor.minY - gap - screen.minY
                let roomAbove = screen.maxY - anchor.maxY - gap
                if edge == .bottom {
                    frame.origin.y = (roomBelow < size.height && roomAbove > roomBelow) ? above : below
                } else {
                    frame.origin.y = (roomAbove < size.height && roomBelow > roomAbove) ? below : above
                }
            case .leading, .trailing:
                // Vertical alignment: leading = tops aligned.
                let top = Self.align(alignment, start: -anchor.maxY, mid: -anchor.midY, end: -anchor.minY, length: size.height)
                frame.origin.y = -top - size.height
                let right = anchor.maxX + gap
                let left = anchor.minX - gap - size.width
                let roomRight = screen.maxX - anchor.maxX - gap
                let roomLeft = anchor.minX - gap - screen.minX
                if edge == .trailing {
                    frame.origin.x = (roomRight < size.width && roomLeft > roomRight) ? left : right
                } else {
                    frame.origin.x = (roomLeft < size.width && roomRight > roomLeft) ? right : left
                }
            }
        case .point(let point):
            frame.origin = CGPoint(x: point.x, y: point.y - size.height)
            if frame.minY < screen.minY + Self.screenMargin, point.y + size.height <= screen.maxY - Self.screenMargin {
                frame.origin.y = point.y
            }
            if frame.maxX > screen.maxX - Self.screenMargin, point.x - size.width >= screen.minX + Self.screenMargin {
                frame.origin.x = point.x - size.width
            }
        case .aligning(let contentPoint, let screenPoint):
            frame.origin = CGPoint(x: screenPoint.x - contentPoint.x, y: screenPoint.y + contentPoint.y - size.height)
        case .submenu(let parent, let top, let gap):
            frame.origin = CGPoint(x: parent.maxX + gap, y: top - size.height)
            if frame.maxX > screen.maxX - Self.screenMargin, parent.minX - gap - size.width >= screen.minX + Self.screenMargin {
                frame.origin.x = parent.minX - gap - size.width
            }
        case .centered(let topFraction):
            frame.origin.x = (screen.midX - size.width / 2).rounded()
            if let topFraction {
                frame.origin.y = (screen.maxY - screen.height * topFraction - size.height).rounded()
            } else {
                frame.origin.y = (screen.midY - size.height / 2).rounded()
            }
        }
        return Self.clamp(frame, to: screen)
    }

    /// Keeps `frame` on `screen`, a margin in from its edges (top-left wins when it can't fit).
    static func clamp(_ frame: CGRect, to screen: CGRect) -> CGRect {
        let inner = screen.insetBy(dx: screenMargin, dy: screenMargin)
        var frame = frame
        frame.origin.x = min(frame.origin.x, inner.maxX - frame.width)
        frame.origin.x = max(frame.origin.x, inner.minX)
        frame.origin.y = max(frame.origin.y, inner.minY)
        frame.origin.y = min(frame.origin.y, inner.maxY - frame.height)
        return frame
    }

    private static func align(_ alignment: Alignment, start: CGFloat, mid: CGFloat, end: CGFloat, length: CGFloat) -> CGFloat {
        switch alignment {
        case .leading: start
        case .center: (mid - length / 2).rounded()
        case .trailing: end - length
        }
    }
}
