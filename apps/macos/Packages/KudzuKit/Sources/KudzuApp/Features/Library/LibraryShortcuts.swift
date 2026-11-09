// The cheat sheet (`shortcuts.tsx`). Combos are the web's strings so `Shortcut` draws the same keycaps.

import Foundation

struct ShortcutEntry: Equatable, Identifiable, Sendable {
    var combo: String
    var label: String
    var id: String { combo + "\u{1f}" + label }
}

struct ShortcutSection: Equatable, Identifiable, Sendable {
    var title: String
    var items: [ShortcutEntry]
    var id: String { title }
}

enum LibraryShortcuts {
    /// The web bounds its scrolling content to min(640px, 75vh).
    static func contentHeight(viewportHeight: Double) -> Double { min(640, viewportHeight * 0.75) }

    static let sections: [ShortcutSection] = [
        ShortcutSection(title: "Everywhere", items: [
            ShortcutEntry(combo: "mod+k", label: "Search & commands"),
            ShortcutEntry(combo: "mod+n", label: "New snippet"),
            ShortcutEntry(combo: "mod+shift+n", label: "New group"),
            ShortcutEntry(combo: "mod+f", label: "Filter snippets"),
            ShortcutEntry(combo: "mod+shift+p", label: "Pause / resume expanding"),
            ShortcutEntry(combo: "mod+,", label: "Settings"),
            ShortcutEntry(combo: "mod+/", label: "This cheat sheet"),
        ]),
        ShortcutSection(title: "Navigate", items: [
            ShortcutEntry(combo: "mod+1", label: "All snippets"),
            ShortcutEntry(combo: "mod+2", label: "Favorites"),
            ShortcutEntry(combo: "mod+3", label: "Recently used"),
            ShortcutEntry(combo: "mod+4", label: "Insights"),
            ShortcutEntry(combo: "mod+5", label: "Clipboard history"),
            ShortcutEntry(combo: "mod+l", label: "Focus the list"),
            ShortcutEntry(combo: "mod+e", label: "Edit content"),
            ShortcutEntry(combo: "escape", label: "Back to the list"),
        ]),
        ShortcutSection(title: "Snippet list", items: [
            ShortcutEntry(combo: "up", label: "Previous (or K)"),
            ShortcutEntry(combo: "down", label: "Next (or J)"),
            ShortcutEntry(combo: "shift+down", label: "Extend selection"),
            ShortcutEntry(combo: "mod+a", label: "Select all"),
            ShortcutEntry(combo: "enter", label: "Open in editor"),
            ShortcutEntry(combo: "/", label: "Filter"),
            ShortcutEntry(combo: "mod+backspace", label: "Delete (undoable)"),
        ]),
        ShortcutSection(title: "Clipboard history", items: [
            ShortcutEntry(combo: "enter", label: "Copy"),
            ShortcutEntry(combo: "shift+enter", label: "Copy as plain text"),
            ShortcutEntry(combo: "space", label: "Large preview"),
            ShortcutEntry(combo: "mod+p", label: "Pin / unpin"),
            ShortcutEntry(combo: "mod+s", label: "Save as snippet"),
            ShortcutEntry(combo: "mod+o", label: "Open link or file"),
            ShortcutEntry(combo: "mod+backspace", label: "Delete (undoable)"),
        ]),
        ShortcutSection(title: "Editor", items: [
            ShortcutEntry(combo: "/", label: "Insert a variable"),
            ShortcutEntry(combo: "mod+d", label: "Duplicate snippet"),
            ShortcutEntry(combo: "mod+shift+f", label: "Toggle favorite"),
            ShortcutEntry(combo: "mod+shift+e", label: "Turn on / off"),
            ShortcutEntry(combo: "mod+z", label: "Undo"),
            ShortcutEntry(combo: "mod+s", label: "Save now (auto-saves anyway)"),
        ]),
    ]
}
