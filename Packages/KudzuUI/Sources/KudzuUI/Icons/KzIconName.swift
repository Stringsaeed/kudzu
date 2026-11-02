import Foundation

/// One drawing instruction in a 16-unit icon glyph.
public enum KzGlyphPart: Hashable, Sendable {
    /// SVG path data, stroked at 1.5 units with round caps and joins.
    case stroke(String)
    /// SVG path data, filled (only the favourite star uses this).
    case fill(String)
    /// A stroked circle (`c:x:y:r` in the HTML reference).
    case circle(x: Double, y: Double, r: Double)
}

/// Every glyph in Kudzu's icon set: 16-unit grid, 1.5 strokes, round caps and joins.
///
/// The first 33 cases are ported verbatim from the design system (`bundle.js` `ICONS`).
/// The cases under "Additions" were drawn for the app in the same style; their SVGs live in
/// `design/kudzu/additions/icons/` so they can go back into the design system.
public enum KzIconName: String, CaseIterable, Sendable, Identifiable {
    // Design system set.
    /// Magnifier: search fields and the search palette.
    case search
    /// Plus: add, new.
    case plus
    /// Outlined star: "make favourite" when not a favourite.
    case star
    /// Filled star: favourites, always in `bloom`.
    case starFill = "star-fill"
    /// Clock: recently used.
    case clock
    /// Bar chart: stats.
    case chart
    /// Folder: a group.
    case folder
    /// Folder with plus: new group.
    case folderPlus = "folder-plus"
    /// Stacked layers: all snippets.
    case stack
    /// Sliders: settings (Kudzu's gear).
    case sliders
    /// Cloud: sync.
    case cloud
    /// Counter-clockwise clock: backups, history, restore.
    case history
    /// Up and down arrows: sort.
    case sort
    /// Arrow into a tray: import and export.
    case tray
    /// Circled i: information.
    case info
    /// Chevron pointing down: menus, expanded disclosures.
    case chevronDown = "chevron-down"
    /// Chevron pointing right: collapsed disclosures, navigation.
    case chevronRight = "chevron-right"
    /// Up/down chevrons: select controls.
    case chevronsV = "chevrons-v"
    /// Cross: close, remove, clear.
    case x
    /// Check mark: menu checks, done.
    case check
    /// Minus: remove, stepper decrement.
    case minus
    /// Square with pencil: new snippet, edit.
    case compose
    /// Bug: debug.
    case bug
    /// Window with sidebar: toggle the sidebar.
    case sidebar
    /// Lightning bolt: expansions, use counts.
    case bolt
    /// Text glyph: plain-text content type.
    case type
    /// Stopwatch: time saved.
    case timer
    /// Curly braces: template tags.
    case braces
    /// Keyboard: shortcuts, typing stats.
    case keyboard
    /// Bin: delete.
    case trash
    /// Leaf: Kudzu itself, empty states.
    case leaf
    /// Three dots: more actions.
    case more
    /// App window: per-app rules.
    case app

    // Additions drawn for the app (see design/kudzu/additions/icons).
    /// Triangle with exclamation: warnings (always with a word).
    case warning
    /// Octagon with exclamation: errors and failures (always with a word).
    case error
    /// Circle with exclamation: a field-level problem.
    case alert
    /// Circled check: success, granted, all set.
    case checkCircle = "check-circle"
    /// Circled minus: remove a row from a list.
    case minusCircle = "minus-circle"
    /// Shield: privacy and permissions.
    case shield
    /// Padlock: Secure Input, locked.
    case lock
    /// Two bars: pause expansion.
    case pause
    /// Triangle: resume expansion.
    case play
    /// Two sheets: copy.
    case copy
    /// I-beam text cursor: insertion point, typing.
    case cursor
    /// Three bars: drag handle for reordering.
    case grip
    /// Arrow out of a box: opens outside Kudzu.
    case external
    /// Lines with a plus: insert text, new snippet from text.
    case textPlus = "text-plus"
    /// Bulleted list: list views, choices.
    case list
    /// Folder with minus: remove a group.
    case folderMinus = "folder-minus"
    /// Arrow turning down and right: inherited from the group.
    case inherit

    /// Stable identity for lists.
    public var id: String { rawValue }

    /// Whether this glyph is one of the app's additions rather than the design system's original 33.
    public var isAddition: Bool {
        KzIconName.allCases.firstIndex(of: self)! >= KzIconName.allCases.firstIndex(of: .warning)!
    }

    /// A default VoiceOver label, for icons that carry meaning without visible text.
    public var accessibilityLabel: String {
        switch self {
        case .search: "Search"
        case .plus: "Add"
        case .star: "Favorite"
        case .starFill: "Favorite"
        case .clock: "Recently used"
        case .chart: "Stats"
        case .folder: "Group"
        case .folderPlus: "New group"
        case .stack: "All snippets"
        case .sliders: "Settings"
        case .cloud: "Sync"
        case .history: "History"
        case .sort: "Sort"
        case .tray: "Import and export"
        case .info: "Information"
        case .chevronDown: "Expand"
        case .chevronRight: "Show more"
        case .chevronsV: "Choose"
        case .x: "Remove"
        case .check: "Selected"
        case .minus: "Decrease"
        case .compose: "New snippet"
        case .bug: "Debug"
        case .sidebar: "Sidebar"
        case .bolt: "Expansions"
        case .type: "Text"
        case .timer: "Time saved"
        case .braces: "Template"
        case .keyboard: "Keyboard"
        case .trash: "Delete"
        case .leaf: "Kudzu"
        case .more: "More"
        case .app: "App"
        case .warning: "Warning"
        case .error: "Error"
        case .alert: "Problem"
        case .checkCircle: "Done"
        case .minusCircle: "Remove"
        case .shield: "Privacy"
        case .lock: "Locked"
        case .pause: "Pause"
        case .play: "Resume"
        case .copy: "Copy"
        case .cursor: "Text cursor"
        case .grip: "Reorder"
        case .external: "Open"
        case .textPlus: "Insert text"
        case .list: "List"
        case .folderMinus: "Remove group"
        case .inherit: "Inherited"
        }
    }

    /// The glyph's drawing instructions on the 16-unit grid.
    public var parts: [KzGlyphPart] {
        switch self {
        // Verbatim from bundle.js ICONS.
        case .search: [.circle(x: 7, y: 7, r: 4.5), .stroke("M10.5 10.5 14 14")]
        case .plus: [.stroke("M8 3v10"), .stroke("M3 8h10")]
        case .star: [.stroke("M8 1.9l1.85 3.76 4.15.6-3 2.93.71 4.13L8 11.38l-3.71 1.94.71-4.13-3-2.93 4.15-.6z")]
        case .starFill: [.fill("M8 1.9l1.85 3.76 4.15.6-3 2.93.71 4.13L8 11.38l-3.71 1.94.71-4.13-3-2.93 4.15-.6z")]
        case .clock: [.circle(x: 8, y: 8, r: 6), .stroke("M8 4.75V8l2.25 1.5")]
        case .chart: [.stroke("M3 13.25V8.5"), .stroke("M8 13.25V2.75"), .stroke("M13 13.25V6")]
        case .folder: [.stroke("M2 4.5A1.5 1.5 0 0 1 3.5 3h2.6l1.4 1.5h5A1.5 1.5 0 0 1 14 6v5.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z")]
        case .folderPlus: [.stroke("M2 4.5A1.5 1.5 0 0 1 3.5 3h2.6l1.4 1.5h5A1.5 1.5 0 0 1 14 6v5.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z"), .stroke("M8 6.75v4"), .stroke("M6 8.75h4")]
        case .stack: [.stroke("M8 2.25 2.25 5 8 7.75 13.75 5z"), .stroke("M2.25 8 8 10.75 13.75 8"), .stroke("M2.25 11 8 13.75 13.75 11")]
        case .sliders: [.stroke("M2.75 5h6"), .stroke("M12.25 5h1"), .stroke("M2.75 11h1"), .stroke("M7.25 11h6"), .circle(x: 10.5, y: 5, r: 1.75), .circle(x: 5.5, y: 11, r: 1.75)]
        case .cloud: [.stroke("M4.75 12.75h6.75a3 3 0 0 0 .35-5.98A4 4 0 0 0 4.3 7.7a2.55 2.55 0 0 0 .45 5.05z")]
        case .history: [.stroke("M2.75 8a5.25 5.25 0 1 0 1.55-3.72"), .stroke("M2.5 2.5v2.75h2.75"), .stroke("M8 5.25V8l2 1.25")]
        case .sort: [.stroke("M5 2.75v10.5"), .stroke("M2.5 5.25 5 2.75l2.5 2.5"), .stroke("M11 13.25V2.75"), .stroke("M8.5 10.75 11 13.25l2.5-2.5")]
        case .tray: [.stroke("M8 2.5v7"), .stroke("M5.25 6.75 8 9.5l2.75-2.75"), .stroke("M2.75 10.5v1.75a1.25 1.25 0 0 0 1.25 1.25h8a1.25 1.25 0 0 0 1.25-1.25V10.5")]
        case .info: [.circle(x: 8, y: 8, r: 6), .stroke("M8 7.25V11"), .stroke("M8 5h.01")]
        case .chevronDown: [.stroke("M4.5 6.25 8 9.75l3.5-3.5")]
        case .chevronRight: [.stroke("M6.25 4.5 9.75 8l-3.5 3.5")]
        case .chevronsV: [.stroke("M5.5 6 8 3.5 10.5 6"), .stroke("M5.5 10 8 12.5l2.5-2.5")]
        case .x: [.stroke("M4.5 4.5l7 7"), .stroke("M11.5 4.5l-7 7")]
        case .check: [.stroke("M3.5 8.25 6.5 11.25 12.5 5")]
        case .minus: [.stroke("M3.5 8h9")]
        case .compose: [.stroke("M7.25 3H4.5A1.5 1.5 0 0 0 3 4.5v7A1.5 1.5 0 0 0 4.5 13h7a1.5 1.5 0 0 0 1.5-1.5V8.75"), .stroke("M11.25 2.5l2.25 2.25L8.25 10H6V7.75z")]
        case .bug: [.stroke("M5.5 7a2.5 2.5 0 0 1 5 0v2.5a2.5 2.5 0 0 1-5 0z"), .stroke("M6.25 4.75 5.25 3.25"), .stroke("M9.75 4.75l1-1.5"), .stroke("M2.75 8.25H5.5"), .stroke("M10.5 8.25h2.75"), .stroke("M3.25 12l2.4-1.2"), .stroke("M12.75 12l-2.4-1.2")]
        case .sidebar: [.stroke("M3.5 3h9A1.5 1.5 0 0 1 14 4.5v7a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5v-7A1.5 1.5 0 0 1 3.5 3z"), .stroke("M6.25 3v10")]
        case .bolt: [.stroke("M9.25 1.75 3.75 9H8l-1.25 5.25L12.25 7H8z")]
        case .type: [.stroke("M2.75 4.25h7"), .stroke("M6.25 4.25v8.5"), .stroke("M9.75 8.5h3.5"), .stroke("M11.5 8.5v4.25")]
        case .timer: [.circle(x: 8, y: 9.25, r: 5), .stroke("M8 6.75v2.5"), .stroke("M6.5 2.25h3")]
        case .braces: [.stroke("M5.75 2.75c-1.25 0-1.75.5-1.75 1.75v1.25c0 1-.5 1.75-1.5 2.25 1 .5 1.5 1.25 1.5 2.25v1.25c0 1.25.5 1.75 1.75 1.75"), .stroke("M10.25 2.75c1.25 0 1.75.5 1.75 1.75v1.25c0 1 .5 1.75 1.5 2.25-1 .5-1.5 1.25-1.5 2.25v1.25c0 1.25-.5 1.75-1.75 1.75")]
        case .keyboard: [.stroke("M3 4h10a1.25 1.25 0 0 1 1.25 1.25v5.5A1.25 1.25 0 0 1 13 12H3a1.25 1.25 0 0 1-1.25-1.25v-5.5A1.25 1.25 0 0 1 3 4z"), .stroke("M4.5 6.5h.01"), .stroke("M7 6.5h.01"), .stroke("M9.5 6.5h.01"), .stroke("M12 6.5h-.5"), .stroke("M5 9.5h6")]
        case .trash: [.stroke("M2.75 4.5h10.5"), .stroke("M6.25 4.5V3h3.5v1.5"), .stroke("M4.25 4.5l.6 8.1a1.5 1.5 0 0 0 1.5 1.4h3.3a1.5 1.5 0 0 0 1.5-1.4l.6-8.1")]
        case .leaf: [.stroke("M2.75 13.25C2.75 7.25 6.5 2.75 13.25 2.75c0 6.75-4.5 10.5-10.5 10.5z"), .stroke("M2.75 13.25 9.25 6.75")]
        case .more: [.stroke("M3.75 8h.01"), .stroke("M8 8h.01"), .stroke("M12.25 8h.01")]
        case .app: [.stroke("M3.5 2.75h9a1.25 1.25 0 0 1 1.25 1.25v8a1.25 1.25 0 0 1-1.25 1.25h-9A1.25 1.25 0 0 1 2.25 12V4A1.25 1.25 0 0 1 3.5 2.75z"), .stroke("M2.25 5.75h11.5")]

        // Additions, same grid and weight.
        case .warning: [.stroke("M7.13 2.75a1 1 0 0 1 1.74 0l5.4 9.5a1 1 0 0 1-.87 1.5H2.6a1 1 0 0 1-.87-1.5z"), .stroke("M8 6.5v2.75"), .stroke("M8 11.25h.01")]
        case .error: [.stroke("M5.6 2h4.8L14 5.6v4.8L10.4 14H5.6L2 10.4V5.6z"), .stroke("M8 5v3.5"), .stroke("M8 11h.01")]
        case .alert: [.circle(x: 8, y: 8, r: 6), .stroke("M8 4.75V8.5"), .stroke("M8 11h.01")]
        case .checkCircle: [.circle(x: 8, y: 8, r: 6), .stroke("M5.5 8.25 7.25 10l3.25-3.75")]
        case .minusCircle: [.circle(x: 8, y: 8, r: 6), .stroke("M5.25 8h5.5")]
        case .shield: [.stroke("M8 1.75 13 3.5v4.25c0 3-2.1 5.35-5 6.5-2.9-1.15-5-3.5-5-6.5V3.5z"), .stroke("M5.75 8 7.25 9.5l3-3")]
        case .lock: [.stroke("M4.25 7.25h7.5A1.25 1.25 0 0 1 13 8.5v4a1.25 1.25 0 0 1-1.25 1.25h-7.5A1.25 1.25 0 0 1 3 12.5v-4a1.25 1.25 0 0 1 1.25-1.25z"), .stroke("M5.25 7.25V5a2.75 2.75 0 0 1 5.5 0v2.25")]
        case .pause: [.stroke("M5.75 3.5v9"), .stroke("M10.25 3.5v9")]
        case .play: [.stroke("M5 3.1v9.8a.6.6 0 0 0 .92.5l7.35-4.9a.6.6 0 0 0 0-1L5.92 2.6A.6.6 0 0 0 5 3.1z")]
        case .copy: [.stroke("M6.75 5.5h5.5A1.25 1.25 0 0 1 13.5 6.75v5.5a1.25 1.25 0 0 1-1.25 1.25h-5.5A1.25 1.25 0 0 1 5.5 12.25v-5.5A1.25 1.25 0 0 1 6.75 5.5z"), .stroke("M10.5 5.5V3.75A1.25 1.25 0 0 0 9.25 2.5h-5.5A1.25 1.25 0 0 0 2.5 3.75v5.5a1.25 1.25 0 0 0 1.25 1.25H5.5")]
        case .cursor: [.stroke("M8 2.75v10.5"), .stroke("M5.75 2.75h4.5"), .stroke("M5.75 13.25h4.5")]
        case .grip: [.stroke("M3 5h10"), .stroke("M3 8h10"), .stroke("M3 11h10")]
        case .external: [.stroke("M9.5 2.5h4v4"), .stroke("M13.5 2.5 8 8"), .stroke("M11.5 9.5v2.5a1.5 1.5 0 0 1-1.5 1.5H4A1.5 1.5 0 0 1 2.5 12V6A1.5 1.5 0 0 1 4 4.5h2.5")]
        case .textPlus: [.stroke("M2.75 4h10.5"), .stroke("M2.75 8h5.5"), .stroke("M2.75 12h4"), .stroke("M11.5 8.75v5"), .stroke("M9 11.25h5")]
        case .list: [.stroke("M6 4.5h7.25"), .stroke("M6 8h7.25"), .stroke("M6 11.5h7.25"), .stroke("M3 4.5h.01"), .stroke("M3 8h.01"), .stroke("M3 11.5h.01")]
        case .folderMinus: [.stroke("M2 4.5A1.5 1.5 0 0 1 3.5 3h2.6l1.4 1.5h5A1.5 1.5 0 0 1 14 6v5.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z"), .stroke("M6 8.75h4")]
        case .inherit: [.stroke("M3.75 2.75v4.5a2.25 2.25 0 0 0 2.25 2.25h7.25"), .stroke("M10.5 6.75l2.75 2.75-2.75 2.75")]
        }
    }
}
