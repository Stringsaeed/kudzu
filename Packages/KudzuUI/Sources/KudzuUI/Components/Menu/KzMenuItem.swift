import SwiftUI

/// A menu: an optional title and its elements. Build one with the `KzMenuBuilder` DSL:
///
/// ```swift
/// KzMenu(title: "Sort by") {
///     KzMenuItem("Name", isChecked: sort == .name) { sort = .name }
///     KzMenuItem("Last used", isChecked: sort == .lastUsed) { sort = .lastUsed }
///     KzMenuSeparator()
///     KzMenuItem("Delete", icon: .trash, role: .danger) { delete() }.keyboardShortcut(.delete)
/// }
/// ```
public struct KzMenu {
    /// The `caption` title above the items, if any.
    public var title: String?
    /// Items, separators and section headers, top to bottom.
    public var elements: [KzMenuElement]

    /// Creates a menu from the DSL.
    public init(title: String? = nil, @KzMenuBuilder _ content: () -> [KzMenuElement]) {
        self.title = title
        self.elements = content()
    }

    /// Creates a menu from a list of elements.
    public init(title: String? = nil, elements: [KzMenuElement]) {
        self.title = title
        self.elements = elements
    }

    /// Every item in the menu, depth first, including those in submenus.
    public var allItems: [KzMenuItem] {
        elements.flatMap { element -> [KzMenuItem] in
            guard case .item(let item) = element else { return [] }
            return [item] + (item.submenu?.allItems ?? [])
        }
    }
}

/// One row of a menu.
public enum KzMenuElement {
    /// A selectable item (possibly opening a submenu).
    case item(KzMenuItem)
    /// A 1px `line` between groups of items.
    case separator
    /// A `caption` heading inside the menu, for grouping (not selectable).
    case header(String)

    /// The item, if this element is one.
    public var item: KzMenuItem? {
        if case .item(let item) = self { return item }
        return nil
    }
}

/// How an item reads.
public enum KzMenuItemRole: Sendable {
    /// `ink` text.
    case standard
    /// A destructive action: `danger` text and check.
    case danger
}

/// A menu item: label, optional icon, check state, shortcut, role, and the action it runs.
///
/// The leading 16pt column shows the `vine-ink` check when `isChecked == true`, otherwise the icon.
/// Use `isChecked` (true or false) for items that are part of a choice, so VoiceOver reads them as
/// checkable; leave it `nil` for plain commands.
public struct KzMenuItem: Identifiable {
    /// Identity. Unique by default, so items with the same label stay distinct; pass `id` to refer to one.
    public var id: String
    /// The text.
    public var label: String
    /// A 14pt icon in the leading column (hidden while checked).
    public var icon: KzIconName?
    /// `nil` for a plain command; `true`/`false` for a checkable choice.
    public var isChecked: Bool?
    /// The key equivalent shown at the trailing edge in Mac glyphs. It also activates the item while the menu is open.
    public var shortcut: KeyboardShortcut?
    /// `standard` or `danger`.
    public var role: KzMenuItemRole
    /// Disabled items are drawn in `ink-faint` and skipped by the keyboard.
    public var isEnabled: Bool
    /// Items of a submenu that opens beside this item; the action is not used when set.
    public var submenu: KzMenu?
    /// Runs after the menu has closed.
    public var action: @MainActor () -> Void

    /// Creates an item.
    public init(
        _ label: String,
        icon: KzIconName? = nil,
        isChecked: Bool? = nil,
        shortcut: KeyboardShortcut? = nil,
        role: KzMenuItemRole = .standard,
        isEnabled: Bool = true,
        id: String? = nil,
        action: @escaping @MainActor () -> Void
    ) {
        self.id = id ?? UUID().uuidString
        self.label = label
        self.icon = icon
        self.isChecked = isChecked
        self.shortcut = shortcut
        self.role = role
        self.isEnabled = isEnabled
        self.submenu = nil
        self.action = action
    }

    /// True when the item opens a submenu.
    public var hasSubmenu: Bool { submenu != nil }

    /// A copy with the given enabled state.
    public func disabled(_ disabled: Bool = true) -> KzMenuItem {
        var copy = self
        copy.isEnabled = !disabled
        return copy
    }

    /// A copy with the given check state.
    public func checked(_ checked: Bool) -> KzMenuItem {
        var copy = self
        copy.isChecked = checked
        return copy
    }

    /// A copy showing (and responding to) a key equivalent; modifiers default to ⌘.
    public func keyboardShortcut(_ key: KeyEquivalent, modifiers: EventModifiers = .command) -> KzMenuItem {
        var copy = self
        copy.shortcut = KeyboardShortcut(key, modifiers: modifiers)
        return copy
    }
}

// MARK: - DSL

/// Anything that can sit in a `KzMenuBuilder` block.
public protocol KzMenuContent {
    /// The rows this produces.
    var menuElements: [KzMenuElement] { get }
}

extension KzMenuItem: KzMenuContent {
    public var menuElements: [KzMenuElement] { [.item(self)] }
}

extension KzMenuElement: KzMenuContent {
    public var menuElements: [KzMenuElement] { [self] }
}

/// A separator line between groups of items.
public struct KzMenuSeparator: KzMenuContent {
    /// Creates a separator.
    public init() {}
    public var menuElements: [KzMenuElement] { [.separator] }
}

/// A `caption` heading inside a menu, grouping the items below it.
public struct KzMenuHeader: KzMenuContent {
    /// The heading text.
    public let title: String
    /// Creates a heading.
    public init(_ title: String) { self.title = title }
    public var menuElements: [KzMenuElement] { [.header(title)] }
}

/// An item that opens a submenu beside it.
public struct KzSubmenu: KzMenuContent {
    private var item: KzMenuItem

    /// Creates a submenu item.
    public init(_ label: String, icon: KzIconName? = nil, isEnabled: Bool = true, id: String? = nil, @KzMenuBuilder _ content: () -> [KzMenuElement]) {
        item = KzMenuItem(label, icon: icon, isEnabled: isEnabled, id: id) {}
        item.submenu = KzMenu(elements: content())
    }

    public var menuElements: [KzMenuElement] { [.item(item)] }
}

/// Builds menu elements from items, separators, headers and submenus, with `if`, `switch` and `for`.
@resultBuilder
public enum KzMenuBuilder {
    public static func buildExpression(_ content: some KzMenuContent) -> [KzMenuElement] { content.menuElements }
    public static func buildExpression(_ elements: [KzMenuElement]) -> [KzMenuElement] { elements }
    public static func buildExpression(_ items: [KzMenuItem]) -> [KzMenuElement] { items.map { .item($0) } }
    public static func buildBlock(_ parts: [KzMenuElement]...) -> [KzMenuElement] { parts.flatMap { $0 } }
    public static func buildArray(_ parts: [[KzMenuElement]]) -> [KzMenuElement] { parts.flatMap { $0 } }
    public static func buildOptional(_ part: [KzMenuElement]?) -> [KzMenuElement] { part ?? [] }
    public static func buildEither(first part: [KzMenuElement]) -> [KzMenuElement] { part }
    public static func buildEither(second part: [KzMenuElement]) -> [KzMenuElement] { part }
    public static func buildLimitedAvailability(_ part: [KzMenuElement]) -> [KzMenuElement] { part }
}

// MARK: - Shortcut glyphs

extension KeyboardShortcut {
    /// The shortcut in Mac menu glyphs, modifiers in the system order: `⌃⌥⇧⌘N`.
    public var kzGlyphs: String {
        var out = ""
        if modifiers.contains(.control) { out += "⌃" }
        if modifiers.contains(.option) { out += "⌥" }
        if modifiers.contains(.shift) { out += "⇧" }
        if modifiers.contains(.command) { out += "⌘" }
        return out + KzMenuKeyGlyph.glyph(for: key)
    }

    /// The shortcut spelled out for VoiceOver: "Command Shift N".
    public var kzSpokenName: String {
        var parts: [String] = []
        if modifiers.contains(.control) { parts.append("Control") }
        if modifiers.contains(.option) { parts.append("Option") }
        if modifiers.contains(.shift) { parts.append("Shift") }
        if modifiers.contains(.command) { parts.append("Command") }
        parts.append(KzMenuKeyGlyph.spokenName(for: key))
        return parts.joined(separator: " ")
    }

    /// True when a key press with these characters (ignoring modifiers) and modifiers is this shortcut.
    func matches(charactersIgnoringModifiers characters: String, modifiers pressed: EventModifiers) -> Bool {
        let relevant: EventModifiers = [.command, .option, .control, .shift]
        guard pressed.intersection(relevant) == modifiers.intersection(relevant) else { return false }
        return Self.normalize(characters) == Self.normalize(String(key.character))
    }

    /// The Delete key types DEL (0x7F) but `KeyEquivalent.delete` is backspace (0x08); keypad Enter
    /// types 0x03 where `.return` is CR.
    private static func normalize(_ characters: String) -> String {
        switch characters {
        case "\u{7F}": "\u{8}"
        case "\u{3}": "\r"
        default: characters.lowercased()
        }
    }
}

enum KzMenuKeyGlyph {
    private static let special: [Character: (glyph: String, spoken: String)] = [
        KeyEquivalent.return.character: ("↩", "Return"),
        KeyEquivalent.delete.character: ("⌫", "Delete"),
        KeyEquivalent.deleteForward.character: ("⌦", "Forward Delete"),
        KeyEquivalent.escape.character: ("⎋", "Escape"),
        KeyEquivalent.tab.character: ("⇥", "Tab"),
        KeyEquivalent.space.character: ("Space", "Space"),
        KeyEquivalent.upArrow.character: ("↑", "Up Arrow"),
        KeyEquivalent.downArrow.character: ("↓", "Down Arrow"),
        KeyEquivalent.leftArrow.character: ("←", "Left Arrow"),
        KeyEquivalent.rightArrow.character: ("→", "Right Arrow"),
        KeyEquivalent.home.character: ("↖", "Home"),
        KeyEquivalent.end.character: ("↘", "End"),
        KeyEquivalent.pageUp.character: ("⇞", "Page Up"),
        KeyEquivalent.pageDown.character: ("⇟", "Page Down"),
        KeyEquivalent.clear.character: ("⌧", "Clear"),
    ]

    static func glyph(for key: KeyEquivalent) -> String {
        special[key.character]?.glyph ?? String(key.character).uppercased()
    }

    static func spokenName(for key: KeyEquivalent) -> String {
        if let name = special[key.character]?.spoken { return name }
        switch key.character {
        case ",": return "Comma"
        case ".": return "Period"
        case "/": return "Slash"
        default: return String(key.character).uppercased()
        }
    }
}
