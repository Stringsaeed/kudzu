import AppKit
@testable import KudzuUI
import SwiftUI
import Testing

/// Rows for: Title? | Name | Date | --- | (disabled) Delete | Duplicate | Export ▸ | ## Group | Dates
private let sampleRows: [KzMenuNavigator.Row] = [
    .item("Name"),
    .item("Date"),
    .inert,
    .item("Delete", isEnabled: false),
    .item("Duplicate"),
    .item("Export", hasSubmenu: true),
    .inert,
    .item("Dates"),
]

@MainActor
@Suite struct MenuNavigationTests {
    @Test func downFromNothingHighlightsFirstSelectable() {
        var nav = KzMenuNavigator(rows: [.inert, .item("A", isEnabled: false), .item("B"), .item("C")])
        #expect(nav.handle(.down, at: 0) == .highlight(2))
    }

    @Test func upFromNothingHighlightsLastSelectable() {
        var nav = KzMenuNavigator(rows: sampleRows)
        #expect(nav.handle(.up, at: 0) == .highlight(7))
    }

    @Test func arrowsSkipSeparatorsHeadersAndDisabledItems() {
        var nav = KzMenuNavigator(rows: sampleRows, highlighted: 1)
        #expect(nav.handle(.down, at: 0) == .highlight(4)) // over the separator and disabled Delete
        #expect(nav.handle(.down, at: 0) == .highlight(5))
        #expect(nav.handle(.down, at: 0) == .highlight(7)) // over the header
        #expect(nav.handle(.up, at: 0) == .highlight(5))
        #expect(nav.handle(.up, at: 0) == .highlight(4))
        #expect(nav.handle(.up, at: 0) == .highlight(1))
    }

    @Test func arrowsWrapAround() {
        var nav = KzMenuNavigator(rows: sampleRows, highlighted: 7)
        #expect(nav.handle(.down, at: 0) == .highlight(0))
        #expect(nav.handle(.up, at: 0) == .highlight(7))
    }

    @Test func homeAndEnd() {
        var nav = KzMenuNavigator(rows: sampleRows, highlighted: 4)
        #expect(nav.handle(.home, at: 0) == .highlight(0))
        #expect(nav.handle(.end, at: 0) == .highlight(7))
    }

    @Test func noSelectableRows() {
        var nav = KzMenuNavigator(rows: [.inert, .item("Off", isEnabled: false)])
        #expect(nav.handle(.down, at: 0) == .highlight(nil))
        #expect(nav.handle(.enter, at: 0) == .dismiss)
    }

    @Test func returnAndSpaceActivateTheHighlightedItem() {
        var nav = KzMenuNavigator(rows: sampleRows, highlighted: 4)
        #expect(nav.handle(.enter, at: 0) == .activate(4))
        #expect(nav.handle(.space, at: 0) == .activate(4))
    }

    @Test func returnWithNothingHighlightedDismisses() {
        var nav = KzMenuNavigator(rows: sampleRows)
        #expect(nav.handle(.enter, at: 0) == .dismiss)
    }

    @Test func escapeDismisses() {
        var nav = KzMenuNavigator(rows: sampleRows, highlighted: 0)
        #expect(nav.handle(.escape, at: 0) == .dismiss)
    }

    @Test func submenus() {
        var nav = KzMenuNavigator(rows: sampleRows, highlighted: 5)
        #expect(nav.handle(.right, at: 0) == .openSubmenu(5))
        #expect(nav.handle(.enter, at: 0) == .openSubmenu(5))
        #expect(nav.handle(.left, at: 0) == .closeSubmenu)
        nav.highlighted = 4
        #expect(nav.handle(.right, at: 0) == .none)
    }

    @Test func navigatorRowsFromElements() {
        let menu = KzMenu(title: "Sort") {
            KzMenuItem("Name") {}
            KzMenuSeparator()
            KzMenuHeader("More")
            KzMenuItem("Off") {}.disabled()
            KzSubmenu("Export") { KzMenuItem("JSON") {} }
        }
        let nav = KzMenuNavigator(elements: menu.elements)
        #expect(nav.rows == [.item("Name"), .inert, .inert, .item("Off", isEnabled: false), .item("Export", hasSubmenu: true)])
        #expect(nav.selectableRows == [0, 4])
    }
}

@MainActor
@Suite struct MenuTypeAheadTests {
    private let rows: [KzMenuNavigator.Row] = [
        .item("Name"), .item("Date created"), .item("Date modified"), .item("Délai", isEnabled: true),
        .item("Disabled thing", isEnabled: false), .item("Usage"),
    ]

    @Test func firstLetterSelectsFirstMatch() {
        var nav = KzMenuNavigator(rows: rows)
        #expect(nav.handle(.character("d"), at: 0) == .highlight(1))
        #expect(nav.handle(.character("u"), at: 5) == .highlight(5)) // after the timeout: a new word
    }

    @Test func prefixNarrows() {
        var nav = KzMenuNavigator(rows: rows)
        _ = nav.handle(.character("D"), at: 0)
        _ = nav.handle(.character("a"), at: 0.1)
        _ = nav.handle(.character("t"), at: 0.2)
        _ = nav.handle(.character("e"), at: 0.3)
        #expect(nav.handle(.space, at: 0.4) == .highlight(1)) // Space extends the word while typing
        _ = nav.handle(.character("m"), at: 0.5)
        #expect(nav.highlighted == 2)
    }

    @Test func repeatedLetterCyclesAndSkipsDisabled() {
        var nav = KzMenuNavigator(rows: rows)
        #expect(nav.handle(.character("d"), at: 0) == .highlight(1))
        #expect(nav.handle(.character("d"), at: 0.1) == .highlight(2))
        #expect(nav.handle(.character("d"), at: 0.2) == .highlight(3)) // "Délai": diacritics fold
        #expect(nav.handle(.character("d"), at: 0.3) == .highlight(1)) // "Disabled thing" is skipped; wraps
    }

    @Test func diacriticAndCaseInsensitive() {
        var nav = KzMenuNavigator(rows: rows)
        _ = nav.handle(.character("D"), at: 0)
        _ = nav.handle(.character("E"), at: 0.1)
        #expect(nav.highlighted == 3)
    }

    @Test func noMatchKeepsHighlight() {
        var nav = KzMenuNavigator(rows: rows, highlighted: 0)
        #expect(nav.handle(.character("z"), at: 0) == .none)
        #expect(nav.highlighted == 0)
    }

    /// NSMenu: when the longer prefix matches nothing, the search starts over from the new letter.
    @Test func missedPrefixRestartsWithTheNewLetter() {
        var nav = KzMenuNavigator(rows: [.item("Name"), .item("Date"), .item("Narrow")])
        #expect(nav.handle(.character("n"), at: 0) == .highlight(0))
        #expect(nav.handle(.character("d"), at: 0.1) == .highlight(1)) // "nd" misses; "d" is Date
        #expect(nav.typeAhead == "d")
        #expect(nav.handle(.character("a"), at: 0.2) == .highlight(1)) // "da" still Date
    }

    /// …and when the new letter alone misses too, the buffer is cleared so the next letter starts fresh.
    @Test func missedLetterClearsTheBuffer() {
        var nav = KzMenuNavigator(rows: [.item("Name"), .item("Date"), .item("Narrow")])
        #expect(nav.handle(.character("n"), at: 0) == .highlight(0))
        #expect(nav.handle(.character("z"), at: 0.1) == .none)
        #expect(nav.typeAhead == "")
        #expect(nav.highlighted == 0)
        #expect(nav.handle(.character("a"), at: 0.2) == .none)
        #expect(nav.handle(.character("d"), at: 0.3) == .highlight(1)) // not "nzad"
        #expect(nav.handle(.character("n"), at: 0.4) == .highlight(0)) // "dn" misses; "n" is Name again
        #expect(nav.handle(.character("a"), at: 0.5) == .highlight(0))
        #expect(nav.handle(.character("r"), at: 0.6) == .highlight(2)) // "nar"
    }

    @Test func pauseStartsANewWord() {
        var nav = KzMenuNavigator(rows: rows)
        _ = nav.handle(.character("d"), at: 0)
        #expect(nav.isTypingAhead(at: 0.5))
        #expect(!nav.isTypingAhead(at: 0 + KzMenuNavigator.typeAheadTimeout))
        #expect(nav.handle(.character("n"), at: 2) == .highlight(0))
        #expect(nav.handle(.space, at: 5) == .activate(0)) // not typing any more: Space activates
    }
}

@MainActor
@Suite struct MenuKeyDecodingTests {
    @Test func decodesNavigationKeys() {
        #expect(KzMenuKey.decode(keyCode: 126, charactersIgnoringModifiers: nil, modifiers: []) == .up)
        #expect(KzMenuKey.decode(keyCode: 125, charactersIgnoringModifiers: nil, modifiers: []) == .down)
        #expect(KzMenuKey.decode(keyCode: 126, charactersIgnoringModifiers: nil, modifiers: .option) == .home)
        #expect(KzMenuKey.decode(keyCode: 115, charactersIgnoringModifiers: nil, modifiers: []) == .home)
        #expect(KzMenuKey.decode(keyCode: 121, charactersIgnoringModifiers: nil, modifiers: []) == .end)
        #expect(KzMenuKey.decode(keyCode: 36, charactersIgnoringModifiers: "\r", modifiers: []) == .enter)
        #expect(KzMenuKey.decode(keyCode: 76, charactersIgnoringModifiers: "\u{3}", modifiers: []) == .enter)
        #expect(KzMenuKey.decode(keyCode: 49, charactersIgnoringModifiers: " ", modifiers: []) == .space)
        #expect(KzMenuKey.decode(keyCode: 53, charactersIgnoringModifiers: "\u{1b}", modifiers: []) == .escape)
        #expect(KzMenuKey.decode(keyCode: 123, charactersIgnoringModifiers: nil, modifiers: []) == .left)
    }

    @Test func decodesTypingAndShortcuts() {
        #expect(KzMenuKey.decode(keyCode: 2, charactersIgnoringModifiers: "d", modifiers: []) == .character("d"))
        #expect(KzMenuKey.decode(keyCode: 2, charactersIgnoringModifiers: "d", modifiers: .shift) == .character("d"))
        #expect(KzMenuKey.decode(keyCode: 2, charactersIgnoringModifiers: "d", modifiers: .command) == .shortcut(characters: "d", modifiers: .command))
        #expect(KzMenuKey.decode(keyCode: 122, charactersIgnoringModifiers: "\u{F704}", modifiers: []) == .other) // F1
    }

    /// ⌘/⌃ with arrows, Home, End, Page Up and Page Down are key equivalents, not movement.
    @Test func commandWithNavigationKeysIsAShortcut() {
        let keys: [(UInt16, KeyEquivalent)] = [
            (126, .upArrow), (125, .downArrow), (123, .leftArrow), (124, .rightArrow),
            (115, .home), (119, .end), (116, .pageUp), (121, .pageDown),
        ]
        for (code, key) in keys {
            let characters = String(key.character)
            for modifiers: EventModifiers in [.command, .control, [.command, .option], [.command, .shift]] {
                #expect(KzMenuKey.decode(keyCode: code, charactersIgnoringModifiers: characters, modifiers: modifiers)
                    == .shortcut(characters: characters, modifiers: modifiers))
            }
            // Some events carry no characters; the key code still says which key it was.
            #expect(KzMenuKey.decode(keyCode: code, charactersIgnoringModifiers: nil, modifiers: .command)
                == .shortcut(characters: characters, modifiers: .command))
            #expect(KeyboardShortcut(key).matches(charactersIgnoringModifiers: characters, modifiers: .command))
        }
        // Without ⌘/⌃ they still move.
        #expect(KzMenuKey.decode(keyCode: 126, charactersIgnoringModifiers: "\u{F700}", modifiers: .option) == .home)
        #expect(KzMenuKey.decode(keyCode: 125, charactersIgnoringModifiers: "\u{F701}", modifiers: .option) == .end)
        #expect(KzMenuKey.decode(keyCode: 123, charactersIgnoringModifiers: "\u{F702}", modifiers: .shift) == .left)
        #expect(KzMenuKey.decode(keyCode: 116, charactersIgnoringModifiers: "\u{F72C}", modifiers: []) == .home)
        #expect(KzMenuKey.decode(keyCode: 119, charactersIgnoringModifiers: "\u{F72B}", modifiers: .option) == .end)
    }

    @Test func shortcutGlyphsAndMatching() {
        #expect(KeyboardShortcut("n").kzGlyphs == "⌘N")
        #expect(KeyboardShortcut("n", modifiers: [.shift, .command]).kzGlyphs == "⇧⌘N")
        #expect(KeyboardShortcut("k", modifiers: [.command, .shift, .option, .control]).kzGlyphs == "⌃⌥⇧⌘K")
        #expect(KeyboardShortcut(.delete).kzGlyphs == "⌘⌫")
        #expect(KeyboardShortcut(.return, modifiers: []).kzGlyphs == "↩")
        #expect(KeyboardShortcut(",").kzGlyphs == "⌘,")
        #expect(KeyboardShortcut("n", modifiers: [.shift, .command]).kzSpokenName == "Shift Command N")
        #expect(KeyboardShortcut("d").matches(charactersIgnoringModifiers: "d", modifiers: .command))
        #expect(!KeyboardShortcut("d").matches(charactersIgnoringModifiers: "d", modifiers: [.command, .shift]))
        #expect(KeyboardShortcut(.delete).matches(charactersIgnoringModifiers: "\u{7f}", modifiers: .command))
    }
}

@MainActor
@Suite struct MenuActivationTests {
    @Test func closesAndRestoresFocusBeforeTheActionRuns() {
        var log: [String] = []
        var finishFlash: (@MainActor () -> Void)?
        KzMenuActivation.run(
            flash: { done in log.append("flash"); finishFlash = done },
            isDismissed: { false },
            close: { log.append("close") },
            restoreFocus: { log.append("restoreFocus") },
            perform: { log.append("perform") }
        )
        // Nothing closes or runs while the item is still flashing.
        #expect(log == ["flash"])
        finishFlash?()
        #expect(log == ["flash", "close", "restoreFocus", "perform"])
    }

    /// A click outside or an app switch during the blink closes the menu; that close wins, so focus
    /// isn't pulled back (maybe out of another app) and the action doesn't run.
    @Test func dismissedDuringTheFlashRunsNothing() {
        var log: [String] = []
        var finishFlash: (@MainActor () -> Void)?
        final class Menu { var isClosed = false }
        let menu = Menu()
        KzMenuActivation.run(
            flash: { done in log.append("flash"); finishFlash = done },
            isDismissed: { menu.isClosed },
            close: { log.append("close") },
            restoreFocus: { log.append("restoreFocus") },
            perform: { log.append("perform") }
        )
        menu.isClosed = true // closed by a click outside while the item blinks
        finishFlash?()
        #expect(log == ["flash"])
    }
}

@MainActor
@Suite struct MenuModelTests {
    @Test func builderSupportsControlFlow() {
        let showExtra = true
        let names = ["One", "Two"]
        let menu = KzMenu(title: "T") {
            for name in names { KzMenuItem(name) {} }
            if showExtra { KzMenuSeparator() }
            if !showExtra { KzMenuItem("Hidden") {} } else { KzMenuHeader("Shown") }
            KzSubmenu("More") {
                KzMenuItem("Deep") {}.keyboardShortcut("d")
            }
        }
        #expect(menu.elements.count == 5)
        #expect(menu.elements.compactMap(\.item).map(\.label) == ["One", "Two", "More"])
        #expect(menu.allItems.map(\.label) == ["One", "Two", "More", "Deep"])
        #expect(menu.allItems.last?.shortcut?.kzGlyphs == "⌘D")
    }

    @Test func itemModifiers() {
        let original = KzMenuItem("Sort", isChecked: false) {}
        let item = original.checked(true).disabled()
        #expect(item.isChecked == true)
        #expect(!item.isEnabled)
        #expect(item.id == original.id) // modifiers keep the identity
    }

    @Test func itemsWithTheSameLabelHaveDistinctIDs() {
        let menu = KzMenu {
            KzMenuItem("Copy") {}
            KzMenuItem("Copy") {}
            KzSubmenu("More") { KzMenuItem("Copy") {} }
        }
        #expect(Set(menu.allItems.map(\.id)).count == 4)
        #expect(KzMenuItem("Copy", id: "copy-a") {}.id == "copy-a")
    }

    /// The shortcut finds the row it matched, not the first row with the same label.
    @Test func shortcutMatchesReportTheirRow() {
        let menu = KzMenu {
            KzMenuItem("Copy") {}
            KzMenuItem("Copy") {}.keyboardShortcut("c")
            KzSubmenu("More") {
                KzMenuItem("Copy") {}
                KzMenuItem("Copy") {}.keyboardShortcut("c", modifiers: [.command, .shift])
            }
            KzSubmenu("Off", isEnabled: false) { KzMenuItem("Paste") {}.keyboardShortcut("v") }
            KzMenuItem("Gone") {}.keyboardShortcut("g").disabled()
        }
        #expect(menu.shortcutMatch(characters: "c", modifiers: .command)?.path == [1])
        #expect(menu.shortcutMatch(characters: "c", modifiers: [.command, .shift])?.path == [2, 1])
        #expect(menu.shortcutMatch(characters: "v", modifiers: .command) == nil) // inside a disabled submenu
        #expect(menu.shortcutMatch(characters: "g", modifiers: .command) == nil)

        // Where that row is on screen, if its menu is open.
        #expect(KzMenuRowLocation(path: [1], parentRows: [nil]) == KzMenuRowLocation(level: 0, row: 1))
        #expect(KzMenuRowLocation(path: [2, 1], parentRows: [nil]) == nil) // submenu closed
        #expect(KzMenuRowLocation(path: [2, 1], parentRows: [nil, 2]) == KzMenuRowLocation(level: 1, row: 1))
        #expect(KzMenuRowLocation(path: [2, 1], parentRows: [nil, 3]) == nil) // another submenu is open
        #expect(KzMenuRowLocation(path: [1], parentRows: [nil, 2]) == KzMenuRowLocation(level: 0, row: 1))
    }

    @Test func surfaceHighlightsByIDOrLabel() {
        let menu = KzMenu {
            KzMenuItem("Name") {}
            KzMenuItem("Date", id: "date") {}
        }
        #expect(KzMenuSurface.highlightedRow(in: menu, id: "Name") == 0)
        #expect(KzMenuSurface.highlightedRow(in: menu, id: "date") == 1)
        #expect(KzMenuSurface.highlightedRow(in: menu, id: nil) == nil)
    }
}

@MainActor
@Suite struct MenuLayoutTests {
    private let layout = KzMenuLayout(hasTitle: true, rows: [.item, .item, .separator, .header, .item])

    @Test func heightsFollowTheCSS() {
        // 4 + title 24 + 28 + 28 + 9 + 24 + 28 + 4
        #expect(layout.contentHeight == 149)
        #expect(KzMenuLayout(hasTitle: false, rows: [.item]).contentHeight == 36)
        #expect(layout.top(of: 0) == 28)
        #expect(layout.top(of: 4) == 117)
        #expect(layout.frame(of: 1, width: 208) == CGRect(x: 4, y: 56, width: 200, height: 28))
    }

    @Test func hitTesting() {
        #expect(layout.row(at: CGPoint(x: 20, y: 10), width: 208) == nil) // title
        #expect(layout.row(at: CGPoint(x: 20, y: 30), width: 208) == 0)
        #expect(layout.row(at: CGPoint(x: 20, y: 85), width: 208) == 2) // separator
        #expect(layout.row(at: CGPoint(x: 20, y: 120), width: 208) == 4)
        #expect(layout.row(at: CGPoint(x: 2, y: 30), width: 208) == nil) // padding
        #expect(layout.row(at: CGPoint(x: 20, y: 147), width: 208) == nil) // bottom padding
    }

    @Test func scrollingRevealsRows() {
        // Viewport of 60: rows 0 (28…56) visible at offset 0; row 4 (117…145) needs scrolling.
        #expect(layout.scrollOffset(revealing: 0, viewportHeight: 60, currentOffset: 0) == 0)
        #expect(layout.scrollOffset(revealing: 4, viewportHeight: 60, currentOffset: 0) == 89)
        #expect(layout.scrollOffset(revealing: 0, viewportHeight: 60, currentOffset: 89) == 24)
    }

    /// The SwiftUI rows must be exactly as tall as the layout, or the pointer would hit the wrong item.
    @Test func renderedRowsMatchTheLayout() {
        let menu = KzMenu(title: "Insert tag") {
            KzMenuItem("Date", icon: .clock) {}
            KzMenuItem("Clipboard", icon: .copy) {}.keyboardShortcut("v", modifiers: [.command, .shift])
            KzMenuSeparator()
            KzMenuHeader("Fields")
            KzSubmenu("Form field", icon: .type) { KzMenuItem("Text") {} }
        }
        let host = NSHostingView(rootView: KzMenuRows(menu: menu, highlighted: nil).fixedSize())
        #expect(host.fittingSize.height == KzMenuLayout(menu).contentHeight)
        #expect(host.fittingSize.width >= KzMenuLayout.minWidth)
    }
}

// MARK: - Snapshots

@MainActor
private enum MenuSamples {
    static var sort: KzMenu {
        KzMenu(title: "Sort by") {
            KzMenuItem("Name", isChecked: false) {}
            KzMenuItem("Last used", isChecked: true) {}
            KzMenuItem("Most used", isChecked: false) {}
            KzMenuItem("Date created", isChecked: false) {}
        }
    }

    static var snippet: KzMenu {
        KzMenu {
            KzMenuItem("Duplicate", icon: .copy) {}.keyboardShortcut("d")
            KzMenuItem("Favorite", icon: .star) {}.keyboardShortcut("f", modifiers: [.command, .shift])
            KzMenuItem("Debug expansion", icon: .bug) {}.disabled()
            KzSubmenu("Move to group", icon: .folder) { KzMenuItem("Work") {} }
            KzMenuSeparator()
            KzMenuItem("Delete", icon: .trash, role: .danger) {}.keyboardShortcut(.delete)
        }
    }

    static var insertTag: KzMenu {
        KzMenu(title: "Insert tag") {
            KzMenuHeader("Dates")
            KzMenuItem("Date", icon: .clock) {}
            KzMenuItem("Time", icon: .timer) {}
            KzMenuHeader("Text")
            KzMenuItem("Clipboard", icon: .copy) {}
            KzMenuItem("Cursor position", icon: .cursor) {}
            KzSubmenu("Form field", icon: .type) { KzMenuItem("Text") {} }
        }
    }
}

@MainActor
@Suite struct MenuSnapshotTests {
    @Test func states() throws {
        try snapshot("menu-states", width: 760) {
            HStack(alignment: .top, spacing: KzSpace.s6) {
                KzMenuSurface(MenuSamples.sort)
                KzMenuSurface(MenuSamples.sort, highlightedID: "Most used")
                KzMenuSurface(MenuSamples.snippet, highlightedID: "Favorite")
            }
            .padding(.bottom, KzSpace.s8)
        }
    }

    @Test func headersAndDanger() throws {
        try snapshot("menu-headers", width: 520) {
            HStack(alignment: .top, spacing: KzSpace.s6) {
                KzMenuSurface(MenuSamples.insertTag, highlightedID: "Clipboard")
                KzMenuSurface(MenuSamples.snippet, highlightedID: "Delete")
            }
            .padding(.bottom, KzSpace.s8)
        }
    }
}
