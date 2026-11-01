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
            close: { log.append("close") },
            restoreFocus: { log.append("restoreFocus") },
            perform: { log.append("perform") }
        )
        // Nothing closes or runs while the item is still flashing.
        #expect(log == ["flash"])
        finishFlash?()
        #expect(log == ["flash", "close", "restoreFocus", "perform"])
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
        let item = KzMenuItem("Sort", isChecked: false) {}.checked(true).disabled()
        #expect(item.isChecked == true)
        #expect(!item.isEnabled)
        #expect(item.id == "Sort")
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
