import CoreGraphics
import Foundation
import SwiftUI

/// A key press in an open menu, decoded from the event so the navigation logic stays testable.
enum KzMenuKey: Equatable {
    case up, down, left, right, home, end
    /// Return or keypad Enter.
    case enter
    case space
    case escape
    case tab
    /// A printable character typed without ⌘/⌃ (type-ahead).
    case character(String)
    /// A key with ⌘ or ⌃: may match an item's shortcut.
    case shortcut(characters: String, modifiers: EventModifiers)
    case other

    /// Decodes a key-down from its virtual key code, characters and modifiers.
    static func decode(keyCode: UInt16, charactersIgnoringModifiers characters: String?, modifiers: EventModifiers) -> KzMenuKey {
        let command = modifiers.contains(.command) || modifiers.contains(.control)
        switch keyCode {
        case 126: return command ? .other : (modifiers.contains(.option) ? .home : .up)
        case 125: return command ? .other : (modifiers.contains(.option) ? .end : .down)
        case 123: return .left
        case 124: return .right
        case 115, 116: return .home // Home, Page Up
        case 119, 121: return .end // End, Page Down
        case 36, 76: return command ? .shortcut(characters: characters ?? "\r", modifiers: modifiers) : .enter
        case 53: return .escape
        case 48: return .tab
        case 49: return command ? .shortcut(characters: " ", modifiers: modifiers) : .space
        default: break
        }
        guard let characters, !characters.isEmpty else { return .other }
        if command { return .shortcut(characters: characters, modifiers: modifiers) }
        // Function keys arrive as private-use characters; they're not type-ahead.
        if characters.unicodeScalars.contains(where: { (0xF700...0xF8FF).contains($0.value) || $0.properties.generalCategory == .control }) {
            return .other
        }
        return .character(characters)
    }
}

/// What an open menu should do in response to an event.
enum KzMenuCommand: Equatable {
    /// Nothing changes.
    case none
    /// The highlight moved (or cleared).
    case highlight(Int?)
    /// Run the item at this row (after closing the menu).
    case activate(Int)
    /// Open the submenu of the item at this row, highlighting its first item.
    case openSubmenu(Int)
    /// Close this submenu and go back to its parent.
    case closeSubmenu
    /// Close the whole menu without running anything.
    case dismiss
}

/// The keyboard model of one menu level: the highlight, arrow movement that skips separators,
/// headers and disabled items, Home/End, activation and type-ahead. Pure, so it is unit-tested.
struct KzMenuNavigator {
    /// A row as far as navigation cares.
    struct Row: Equatable {
        /// The label used for type-ahead (`nil` for separators and headers).
        var label: String?
        var isEnabled: Bool
        var hasSubmenu: Bool

        /// A selectable item.
        static func item(_ label: String, isEnabled: Bool = true, hasSubmenu: Bool = false) -> Row {
            Row(label: label, isEnabled: isEnabled, hasSubmenu: hasSubmenu)
        }

        /// A separator or header.
        static let inert = Row(label: nil, isEnabled: false, hasSubmenu: false)

        var isSelectable: Bool { label != nil && isEnabled }
    }

    /// How long a pause resets type-ahead, as in NSMenu's type select.
    static let typeAheadTimeout: TimeInterval = 1.0

    let rows: [Row]
    /// The highlighted row, if any.
    var highlighted: Int?
    private(set) var typeAhead = ""
    private var lastTypeTime: TimeInterval = -.infinity

    init(rows: [Row], highlighted: Int? = nil) {
        self.rows = rows
        self.highlighted = highlighted
    }

    init(elements: [KzMenuElement], highlighted: Int? = nil) {
        self.init(rows: elements.map { element in
            switch element {
            case .item(let item): .item(item.label, isEnabled: item.isEnabled, hasSubmenu: item.hasSubmenu)
            case .separator, .header: .inert
            }
        }, highlighted: highlighted)
    }

    /// Rows that can be highlighted, in order.
    var selectableRows: [Int] { rows.indices.filter { rows[$0].isSelectable } }

    func isSelectable(_ row: Int) -> Bool {
        rows.indices.contains(row) && rows[row].isSelectable
    }

    /// True while a type-ahead word is being typed (Space then extends it instead of activating).
    func isTypingAhead(at time: TimeInterval) -> Bool {
        !typeAhead.isEmpty && time - lastTypeTime < Self.typeAheadTimeout
    }

    // MARK: Movement

    /// Moves the highlight one selectable row down (or up), wrapping like NSMenu.
    /// With nothing highlighted, ↓ goes to the first row and ↑ to the last.
    mutating func move(down: Bool) -> Int? {
        let selectable = selectableRows
        guard !selectable.isEmpty else { return nil }
        guard let current = highlighted else {
            highlighted = down ? selectable.first : selectable.last
            return highlighted
        }
        if down {
            highlighted = selectable.first { $0 > current } ?? selectable.first
        } else {
            highlighted = selectable.last { $0 < current } ?? selectable.last
        }
        return highlighted
    }

    /// Highlights the first (or last) selectable row.
    mutating func moveToEnd(last: Bool) -> Int? {
        highlighted = last ? selectableRows.last : selectableRows.first
        return highlighted
    }

    // MARK: Keys

    /// Applies a key press at `time` (seconds, monotonic) and says what the menu should do.
    mutating func handle(_ key: KzMenuKey, at time: TimeInterval) -> KzMenuCommand {
        switch key {
        case .down: return .highlight(move(down: true))
        case .up: return .highlight(move(down: false))
        case .home: return .highlight(moveToEnd(last: false))
        case .end: return .highlight(moveToEnd(last: true))
        case .right:
            if let row = highlighted, isSelectable(row), rows[row].hasSubmenu { return .openSubmenu(row) }
            return .none
        case .left:
            return .closeSubmenu
        case .space where isTypingAhead(at: time):
            return typeSelect(" ", at: time).map { .highlight($0) } ?? .none
        case .enter, .space:
            guard let row = highlighted, isSelectable(row) else { return .dismiss }
            return rows[row].hasSubmenu ? .openSubmenu(row) : .activate(row)
        case .escape:
            return .dismiss
        case .character(let text):
            return typeSelect(text, at: time).map { .highlight($0) } ?? .none
        case .tab, .shortcut, .other:
            return .none
        }
    }

    // MARK: Type-ahead

    /// Type select: highlights the first item whose label starts with what has been typed within
    /// the timeout. Typing the same letter again steps through the items that start with it.
    @discardableResult
    mutating func typeSelect(_ text: String, at time: TimeInterval) -> Int? {
        let fresh = time - lastTypeTime >= Self.typeAheadTimeout
        if fresh { typeAhead = "" }
        typeAhead += Self.fold(text)
        lastTypeTime = time

        let buffer = typeAhead
        // "ddd": step through the items starting with "d".
        if buffer.count > 1, let first = buffer.first, buffer.allSatisfy({ $0 == first }) {
            return cycle(String(first))
        }
        // Keep the current highlight while it still matches the longer prefix.
        if !fresh, let current = highlighted, matches(current, prefix: buffer) {
            return current
        }
        guard let match = firstMatch(prefix: buffer, from: 0) else { return nil }
        highlighted = match
        return match
    }

    private mutating func cycle(_ prefix: String) -> Int? {
        let start = highlighted.map { $0 + 1 } ?? 0
        guard let match = firstMatch(prefix: prefix, from: start) else { return nil }
        highlighted = match
        return match
    }

    /// The first selectable row at or after `start` (wrapping) whose label starts with `prefix`.
    private func firstMatch(prefix: String, from start: Int) -> Int? {
        guard !rows.isEmpty else { return nil }
        for offset in 0..<rows.count {
            let row = (start + offset) % rows.count
            if matches(row, prefix: prefix) { return row }
        }
        return nil
    }

    private func matches(_ row: Int, prefix: String) -> Bool {
        guard isSelectable(row), let label = rows[row].label else { return false }
        return Self.fold(label).hasPrefix(prefix)
    }

    private static func fold(_ text: String) -> String {
        text.folding(options: [.caseInsensitive, .diacriticInsensitive, .widthInsensitive], locale: nil)
    }
}

/// The order an activation happens in: the optional selection flash, then the menu closes and
/// focus goes back, and only then the item's action runs. Native menus behave the same way, so an
/// action that opens a sheet or another menu starts from a clean state.
@MainActor
enum KzMenuActivation {
    static func run(
        flash: @escaping @MainActor (_ done: @escaping @MainActor () -> Void) -> Void,
        close: @escaping @MainActor () -> Void,
        restoreFocus: @escaping @MainActor () -> Void,
        perform: @escaping @MainActor () -> Void
    ) {
        flash {
            close()
            restoreFocus()
            perform()
        }
    }
}

// MARK: - Layout

/// The menu surface's geometry, computed from the CSS metrics rather than measured, so pointer
/// hit-testing works for events delivered to other windows (press-drag-release) and is testable.
struct KzMenuLayout: Equatable {
    /// `.kz-menu` padding.
    static let padding: CGFloat = 4
    /// `.kz-menu-item` height.
    static let itemHeight: CGFloat = 28
    /// `.kz-menu-sep`: 1px plus 4px margins.
    static let separatorHeight: CGFloat = 9
    /// `.kz-menu-title`: 6 + 14 + 4.
    static let titleHeight: CGFloat = 24
    /// `.kz-menu` min-width.
    static let minWidth: CGFloat = 208
    /// From the surface's left edge to an item's label: padding 4, item inset 6, check column 16, gap 8.
    static let labelInset: CGFloat = 34

    enum RowKind: Equatable { case item, separator, header }

    let hasTitle: Bool
    let rows: [RowKind]

    init(hasTitle: Bool, rows: [RowKind]) {
        self.hasTitle = hasTitle
        self.rows = rows
    }

    init(_ menu: KzMenu) {
        self.init(hasTitle: menu.title != nil, rows: menu.elements.map {
            switch $0 {
            case .item: .item
            case .separator: .separator
            case .header: .header
            }
        })
    }

    static func height(of kind: RowKind) -> CGFloat {
        switch kind {
        case .item: itemHeight
        case .separator: separatorHeight
        case .header: titleHeight
        }
    }

    /// Total height of the content, padding included.
    var contentHeight: CGFloat {
        Self.padding * 2 + (hasTitle ? Self.titleHeight : 0) + rows.reduce(0) { $0 + Self.height(of: $1) }
    }

    /// The top of `row`, from the top of the content.
    func top(of row: Int) -> CGFloat {
        var y = Self.padding + (hasTitle ? Self.titleHeight : 0)
        for index in 0..<row { y += Self.height(of: rows[index]) }
        return y
    }

    /// The row's frame in content coordinates (origin top-left, y down).
    func frame(of row: Int, width: CGFloat) -> CGRect {
        CGRect(x: Self.padding, y: top(of: row), width: max(0, width - Self.padding * 2), height: Self.height(of: rows[row]))
    }

    /// The row under a point in content coordinates (top-left origin), if any.
    func row(at point: CGPoint, width: CGFloat) -> Int? {
        guard point.x >= Self.padding, point.x < width - Self.padding else { return nil }
        var y = Self.padding + (hasTitle ? Self.titleHeight : 0)
        guard point.y >= y else { return nil }
        for (index, kind) in rows.enumerated() {
            let height = Self.height(of: kind)
            if point.y < y + height { return index }
            y += height
        }
        return nil
    }

    /// The scroll offset that brings `row` fully into a viewport of `viewportHeight`, starting from `offset`.
    func scrollOffset(revealing row: Int, viewportHeight: CGFloat, currentOffset offset: CGFloat) -> CGFloat {
        let maxOffset = max(0, contentHeight - viewportHeight)
        let top = top(of: row) - Self.padding
        let bottom = top + Self.height(of: rows[row]) + Self.padding * 2
        var result = offset
        if top < result { result = top }
        if bottom > result + viewportHeight { result = bottom - viewportHeight }
        return min(max(0, result), maxOffset)
    }
}
