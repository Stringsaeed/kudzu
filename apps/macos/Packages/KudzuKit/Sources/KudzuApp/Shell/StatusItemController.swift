import AppKit
import KudzuCore

/// The menu bar icon and menu. Port of `tray.rs`.
///
/// Four template glyphs from the asset catalog (`MenuBar`, `MenuBarPaused`,
/// `MenuBarSecureInput`, `MenuBarPermissions`); a dot in the top-right corner while a downloaded
/// update waits. The menu opens on left click.
@MainActor
final class StatusItemController: NSObject, NSMenuDelegate {
    /// Which glyph to show (the same set as the original Swift app's).
    enum Glyph: String {
        case normal = "MenuBar"
        case paused = "MenuBarPaused"
        case secureInput = "MenuBarSecureInput"
        case needsPermission = "MenuBarPermissions"
    }

    /// What the menu's items do; the shell implements them.
    struct Actions {
        var togglePause: () -> Void
        var search: () -> Void
        var clipboard: () -> Void
        var fromSelection: () -> Void
        var open: () -> Void
        var settings: () -> Void
        var updates: () -> Void
        var quit: () -> Void
    }

    private let item: NSStatusItem
    private let menu = NSMenu()
    private let statusRow = NSMenuItem(title: "Starting…", action: nil, keyEquivalent: "")
    private let pauseRow: NSMenuItem
    private let searchRow: NSMenuItem
    private let clipboardRow: NSMenuItem
    private let updatesRow: NSMenuItem
    private let actions: Actions
    private let isDev: Bool
    private let hotkeys: () -> (palette: String, clipboard: String)

    private var glyph: Glyph
    private var updateDot = false

    init(isDev: Bool, settings: Settings, actions: Actions, hotkeys: @escaping () -> (palette: String, clipboard: String)) {
        self.isDev = isDev
        self.actions = actions
        self.hotkeys = hotkeys
        item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        pauseRow = NSMenuItem(title: settings.paused ? "Resume Expanding" : "Pause Expanding", action: nil, keyEquivalent: "p")
        pauseRow.keyEquivalentModifierMask = [.command, .shift]
        searchRow = NSMenuItem(title: "Quick Search", action: nil, keyEquivalent: "")
        clipboardRow = NSMenuItem(title: "Clipboard History", action: nil, keyEquivalent: "")
        updatesRow = NSMenuItem(title: "Check for Updates…", action: nil, keyEquivalent: "")
        glyph = settings.paused ? .paused : .normal
        super.init()

        statusRow.isEnabled = false
        let fromSelection = NSMenuItem(title: "New Snippet from Selection", action: nil, keyEquivalent: "")
        let open = NSMenuItem(title: isDev ? "Open Kudzu Dev…" : "Open Kudzu…", action: nil, keyEquivalent: "o")
        let settingsRow = NSMenuItem(title: "Settings…", action: nil, keyEquivalent: ",")
        let quit = NSMenuItem(title: isDev ? "Quit Kudzu Dev" : "Quit Kudzu", action: nil, keyEquivalent: "")

        let wired: [(NSMenuItem, Selector)] = [
            (searchRow, #selector(search)), (clipboardRow, #selector(clipboard)), (fromSelection, #selector(fromSelectionAction)),
            (pauseRow, #selector(togglePause)), (open, #selector(openAction)), (settingsRow, #selector(settingsAction)),
            (updatesRow, #selector(updates)), (quit, #selector(quitAction)),
        ]
        for (row, action) in wired {
            row.target = self
            row.action = action
        }
        menu.autoenablesItems = false
        for row in [statusRow, .separator(), searchRow, clipboardRow, fromSelection, pauseRow, .separator(), open, settingsRow, updatesRow, .separator(), quit] {
            menu.addItem(row)
        }
        menu.delegate = self
        item.menu = menu
        item.button?.imagePosition = .imageLeading
        if isDev { item.button?.title = "Dev" }
        item.button?.toolTip = isDev ? "Kudzu Dev" : "Kudzu"
        setClipboardEnabled(settings.clipboardEnabled)
        refreshHints()
        refreshIcon()
    }

    // MARK: Updates from the app

    /// Reflects engine status in the icon and menu.
    func update(_ status: EngineStatus) {
        glyph = Self.glyph(for: status)
        refreshIcon()
        let summary = Self.summary(status)
        item.button?.toolTip = "Kudzu — \(summary)"
        statusRow.title = summary
        pauseRow.title = status.paused ? "Resume Expanding" : "Pause Expanding"
    }

    /// Mirrors the update state in the menu ("Restart to Update to 0.3.0") and the dot.
    func setUpdate(_ state: UpdateState) {
        switch state {
        case let .ready(version, _, _): updatesRow.title = "Restart to Update to \(version)"
        case let .downloading(version, _, _): updatesRow.title = "Downloading \(version)…"
        case .restarting: updatesRow.title = "Restarting…"
        case .checking: updatesRow.title = "Checking for Updates…"
        default: updatesRow.title = "Check for Updates…"
        }
        if case .ready = state { updateDot = true } else { updateDot = false }
        refreshIcon()
    }

    /// Greys out "Clipboard History" while recording is off.
    func setClipboardEnabled(_ enabled: Bool) {
        clipboardRow.isEnabled = enabled
    }

    /// The shortcuts shown next to "Quick Search" and "Clipboard History".
    func refreshHints() {
        let keys = hotkeys()
        Self.apply(keys.palette, to: searchRow)
        Self.apply(keys.clipboard, to: clipboardRow)
    }

    // MARK: Pure parts (tested)

    static func glyph(for status: EngineStatus) -> Glyph {
        if status.paused { return .paused }
        if status.secureInput { return .secureInput }
        if status.tap == .off || !status.permissions.accessibility { return .needsPermission }
        return .normal
    }

    static func summary(_ status: EngineStatus) -> String {
        if status.paused { return "Paused" }
        if status.secureInput {
            if let app = status.secureInputOwner { return "Blocked — Secure Input in \(app)" }
            return "Blocked — Secure Input is on"
        }
        if status.tap == .off { return "Needs permission to work" }
        return "Expanding"
    }

    /// Shows an accelerator ("Control+Alt+Space") as the item's native key equivalent.
    static func apply(_ accelerator: String, to row: NSMenuItem) {
        guard let parsed = Keys.parseAccelerator(accelerator), let key = keyEquivalent(parsed.keyCode) else {
            row.keyEquivalent = ""
            return
        }
        row.keyEquivalent = key
        var mask: NSEvent.ModifierFlags = []
        if parsed.modifiers.contains(.command) { mask.insert(.command) }
        if parsed.modifiers.contains(.option) { mask.insert(.option) }
        if parsed.modifiers.contains(.control) { mask.insert(.control) }
        if parsed.modifiers.contains(.shift) { mask.insert(.shift) }
        row.keyEquivalentModifierMask = mask
    }

    /// The menu key equivalent for a virtual key code.
    static func keyEquivalent(_ code: UInt16) -> String? {
        let special: [UInt16: Int] = [
            49: 0x20, 36: 0x0D, 48: 0x09, 51: 0x08, 53: 0x1B,
            122: NSF1FunctionKey, 120: NSF2FunctionKey, 99: NSF3FunctionKey, 118: NSF4FunctionKey,
            96: NSF5FunctionKey, 97: NSF6FunctionKey, 98: NSF7FunctionKey, 100: NSF8FunctionKey,
            101: NSF9FunctionKey, 109: NSF10FunctionKey, 103: NSF11FunctionKey, 111: NSF12FunctionKey,
            126: NSUpArrowFunctionKey, 125: NSDownArrowFunctionKey, 123: NSLeftArrowFunctionKey, 124: NSRightArrowFunctionKey,
        ]
        if let scalar = special[code].flatMap(UnicodeScalar.init) { return String(Character(scalar)) }
        guard let name = Keys.codes[code] else { return nil }
        if name.hasPrefix("Key") { return name.dropFirst(3).lowercased() }
        if name.hasPrefix("Digit") { return String(name.dropFirst(5)) }
        let punctuation = [
            "Semicolon": ";", "Comma": ",", "Period": ".", "Slash": "/", "Backslash": "\\", "Quote": "'",
            "BracketLeft": "[", "BracketRight": "]", "Minus": "-", "Equal": "=", "Backquote": "`",
        ]
        return punctuation[name]
    }

    // MARK: Icon

    private func refreshIcon() {
        guard let base = NSImage(named: glyph.rawValue) else {
            item.button?.image = nil
            if !isDev { item.button?.title = "Kudzu" }
            return
        }
        let image = updateDot ? Self.withDot(base) : base
        image.isTemplate = true
        item.button?.image = image
    }

    /// Adds a small filled dot in the top-right corner. Template images use alpha only, so the
    /// dot takes the menu bar's text colour, like the system's own badges. A clear ring separates
    /// it from the glyph underneath.
    static func withDot(_ image: NSImage) -> NSImage {
        let size = image.size
        let result = NSImage(size: size, flipped: true) { rect in
            image.draw(in: rect)
            let r = max(min(size.width, size.height) * 0.17, 1)
            let center = NSPoint(x: size.width - r - 0.25, y: r + 0.25)
            let ring = r * 1.45
            NSGraphicsContext.current?.compositingOperation = .clear
            NSBezierPath(ovalIn: NSRect(x: center.x - ring, y: center.y - ring, width: ring * 2, height: ring * 2)).fill()
            NSGraphicsContext.current?.compositingOperation = .sourceOver
            NSColor.black.setFill()
            NSBezierPath(ovalIn: NSRect(x: center.x - r, y: center.y - r, width: r * 2, height: r * 2)).fill()
            return true
        }
        result.isTemplate = true
        return result
    }

    // MARK: Actions

    @objc private func togglePause() { actions.togglePause() }
    @objc private func search() { actions.search() }
    @objc private func clipboard() { actions.clipboard() }
    @objc private func fromSelectionAction() { actions.fromSelection() }
    @objc private func openAction() { actions.open() }
    @objc private func settingsAction() { actions.settings() }
    @objc private func updates() { actions.updates() }
    @objc private func quitAction() { actions.quit() }
}
