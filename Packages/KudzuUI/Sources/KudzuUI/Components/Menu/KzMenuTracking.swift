import CoreGraphics
import Foundation
import SwiftUI

// Pointer and mouse-button tracking for an open menu, kept pure so `KzMenuSession` only applies
// the results and the rules are unit-tested.

/// Where the pointer is relative to an open menu's levels (root first, deepest submenu last).
enum KzMenuHit: Equatable {
    /// Over this level's glass.
    case level(Int)
    /// In the gap between a menu and its submenu: on the way across, so nothing changes.
    case gap
    /// Outside every level.
    case outside

    /// Locates `point` among the levels' glass frames (screen coordinates).
    static func locate(_ point: CGPoint, in frames: [CGRect]) -> KzMenuHit {
        if let index = frames.lastIndex(where: { $0.contains(point) }) { return .level(index) }
        for index in frames.indices.dropLast() where gap(between: frames[index], and: frames[index + 1])?.contains(point) == true {
            return .gap
        }
        return .outside
    }

    /// The strip between a menu and a submenu beside it, as tall as the edges they share.
    private static func gap(between parent: CGRect, and child: CGRect) -> CGRect? {
        let minY = max(parent.minY, child.minY), maxY = min(parent.maxY, child.maxY)
        guard maxY > minY else { return nil }
        if child.minX >= parent.maxX {
            return CGRect(x: parent.maxX, y: minY, width: child.minX - parent.maxX, height: maxY - minY)
        }
        if child.maxX <= parent.minX {
            return CGRect(x: child.maxX, y: minY, width: parent.minX - child.maxX, height: maxY - minY)
        }
        return nil
    }
}

/// Hover and drag over the open levels: which row each level highlights, and what to do with the
/// pending (delayed) submenu change.
struct KzMenuPointerRouter {
    /// What to do with the delayed submenu open or switch.
    enum SubmenuChange: Equatable {
        /// Leave any pending change alone.
        case keep
        /// Drop any pending change.
        case cancel
        /// Replace the pending change: after `delay`, open the submenu of `row` in `level`, or close
        /// the submenus below `level` if that row has none.
        case schedule(level: Int, row: Int?, delay: Duration)
    }

    /// Hovering a submenu item this long opens it.
    static let openDelay: Duration = .milliseconds(120)
    /// Leaving an open submenu's item for another this long switches submenus (time to cross over).
    static let switchDelay: Duration = .milliseconds(250)

    /// The highlighted row of each level.
    var highlights: [Int?]
    /// The parent row each level was opened from (`nil` for the root).
    let parentRows: [Int?]
    /// True while the pointer is over a level (or crossing a gap).
    private(set) var pointerInside: Bool

    init(highlights: [Int?], parentRows: [Int?], pointerInside: Bool = false) {
        self.highlights = highlights
        self.parentRows = parentRows
        self.pointerInside = pointerInside
    }

    /// The pointer moved to `hit`; `row` is the selectable row under it in that level, if any.
    mutating func pointerMoved(_ hit: KzMenuHit, row: Int?, rowHasSubmenu: Bool, immediate: Bool) -> SubmenuChange {
        let deepest = highlights.count - 1
        switch hit {
        case .gap:
            return .keep
        case .outside:
            // The deepest level loses its highlight and each parent goes back to the item whose
            // submenu is open. A keyboard highlight survives the pointer moving around outside.
            if pointerInside {
                highlights[deepest] = nil
                for index in 0..<deepest { highlights[index] = parentRows[index + 1] }
            }
            pointerInside = false
            return .cancel
        case .level(let index):
            pointerInside = true
            if index < deepest {
                // Back in a parent while its submenu is open. Give the pointer time to cross over before switching.
                highlights[index] = row
                if row == parentRows[index + 1] { return .cancel }
                return .schedule(level: index, row: row, delay: immediate ? .zero : Self.switchDelay)
            }
            if index > 0, let parentRow = parentRows[index] { highlights[index - 1] = parentRow }
            highlights[index] = row
            if let row, rowHasSubmenu {
                return .schedule(level: index, row: row, delay: immediate ? .zero : Self.openDelay)
            }
            return .cancel
        }
    }

    /// True when a change scheduled for `row` in `level` still matches what's highlighted, so it should happen.
    func isCurrent(level: Int, row: Int?) -> Bool {
        highlights.indices.contains(level) && highlights[level] == row
    }
}

/// Mouse-button tracking: whether a release chooses the item under it, closes the menu, or does nothing.
///
/// Like native menu tracking: the press that opened the menu can drag to an item and release to
/// choose it, a click leaves the menu open, and a drag released outside closes it. A press inside
/// an open menu that's released outside closes it too.
struct KzMenuPressTracker {
    enum Release: Equatable {
        case none
        /// Choose the item under the pointer (if it's an item).
        case choose
        case close
    }

    /// Release within this time of opening chooses the item under the pointer, even without moving.
    static let holdToChooseDelay: TimeInterval = 0.4
    /// Movement under this many points is a click, not a drag.
    static let dragThreshold: CGFloat = 3

    private let openedAt: TimeInterval
    private var pressLocation: CGPoint
    /// False for the press that opened the menu.
    private var pressedInside = false
    private var dragged = false

    /// `pressLocation` is the pointer when the menu opened (where the opening press, if any, began).
    init(pressLocation: CGPoint, openedAt: TimeInterval) {
        self.pressLocation = pressLocation
        self.openedAt = openedAt
    }

    /// A button went down over the menu (`inside`). Presses outside close the menu before they get here.
    mutating func mouseDown(at point: CGPoint, inside: Bool) {
        pressLocation = point
        pressedInside = inside
        dragged = false
    }

    mutating func mouseDragged(to point: CGPoint) {
        if hypot(point.x - pressLocation.x, point.y - pressLocation.y) > Self.dragThreshold { dragged = true }
    }

    /// The button came up, over the menu or not, at `time`.
    mutating func mouseUp(insideMenu: Bool, at time: TimeInterval) -> Release {
        defer {
            pressedInside = false
            dragged = false
        }
        if insideMenu {
            return pressedInside || dragged || time - openedAt > Self.holdToChooseDelay ? .choose : .none
        }
        return pressedInside || dragged ? .close : .none
    }
}

/// A row on screen: the open level and the row in it.
struct KzMenuRowLocation: Equatable {
    var level: Int
    var row: Int

    init(level: Int, row: Int) {
        self.level = level
        self.row = row
    }

    /// Where the item at `path` (a row in each menu, from the root down) is on screen, if its menu is
    /// open; `parentRows` are the open levels' parent rows (`nil` for the root).
    init?(path: [Int], parentRows: [Int?]) {
        guard let row = path.last, path.count <= parentRows.count else { return nil }
        for depth in 1..<path.count where parentRows[depth] != path[depth - 1] { return nil }
        self.init(level: path.count - 1, row: row)
    }
}

extension KzMenu {
    /// The first enabled item, depth first through enabled submenus, whose key equivalent matches,
    /// with its path of rows from the root.
    func shortcutMatch(characters: String, modifiers: EventModifiers) -> (item: KzMenuItem, path: [Int])? {
        for (row, element) in elements.enumerated() {
            guard let item = element.item, item.isEnabled else { continue }
            if let submenu = item.submenu {
                if let match = submenu.shortcutMatch(characters: characters, modifiers: modifiers) {
                    return (match.item, [row] + match.path)
                }
            } else if item.shortcut?.matches(charactersIgnoringModifiers: characters, modifiers: modifiers) == true {
                return (item, [row])
            }
        }
        return nil
    }
}
