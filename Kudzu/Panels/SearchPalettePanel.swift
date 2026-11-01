import AppKit
import Carbon
import KudzuCore
import SwiftUI

/// The palette window: a key-capable, non-activating floating panel, so the app the user was
/// typing in stays frontmost and gets the insertion afterwards.
final class SearchPalettePanel: NSPanel {
    /// Gets every key-down first; returns true when it handled the event.
    var keyHandler: ((NSEvent) -> Bool)?

    init(size: NSSize) {
        super.init(contentRect: NSRect(origin: .zero, size: size),
                   styleMask: [.titled, .fullSizeContentView, .nonactivatingPanel],
                   backing: .buffered, defer: true)
        titleVisibility = .hidden
        titlebarAppearsTransparent = true
        for button in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
            standardWindowButton(button)?.isHidden = true
        }
        isFloatingPanel = true
        level = .floating
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        hidesOnDeactivate = false
        isReleasedWhenClosed = false
        isMovable = false
        animationBehavior = .utilityWindow
        isOpaque = false
        backgroundColor = .clear
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    override func sendEvent(_ event: NSEvent) {
        // Intercept before the search field's editor, which would otherwise eat ↑/↓/↵/Esc.
        if event.type == .keyDown, let keyHandler, keyHandler(event) { return }
        super.sendEvent(event)
    }
}

/// Shows, hides and drives the search palette. Owned by `AppModel`.
@MainActor
final class SearchPaletteController: NSObject, NSWindowDelegate {
    static let panelSize = NSSize(width: 760, height: 460)

    let model: SearchPaletteModel
    /// Inserts a snippet into the frontmost app. Called after the palette has closed.
    var onInsert: ((UUID) async -> Void)?
    /// Opens the snippet in the main window.
    var onEdit: ((UUID) -> Void)?

    private var panel: SearchPalettePanel?
    private var isClosing = false

    init(store: KudzuStore) {
        model = SearchPaletteModel(store: store)
    }

    var isVisible: Bool { panel?.isVisible ?? false }

    /// Opens the palette, or closes it when it is already open (hotkey pressed again).
    func toggle() {
        if isVisible { close() } else { show() }
    }

    func show() {
        let panel = panel ?? makePanel()
        model.isActive = true
        model.prepareForShow()
        position(panel)
        panel.makeKeyAndOrderFront(nil)
        panel.contentView?.layoutSubtreeIfNeeded()
        if let field = model.searchField { panel.makeFirstResponder(field) }
    }

    func close() {
        guard let panel, panel.isVisible, !isClosing else { return }
        isClosing = true
        defer { isClosing = false }
        model.isActive = false
        panel.orderOut(nil)
    }

    // MARK: - Actions

    /// ↵: close first so the previous app gets keyboard focus back, then insert.
    func insertSelection() {
        guard let id = model.selectedRow?.id else { return NSSound.beep() }
        close()
        Task { @MainActor in
            // Let the window server hand key focus back to the previous app before typing into it.
            try? await Task.sleep(for: .milliseconds(40))
            await onInsert?(id)
        }
    }

    /// ⌘↵: copy the rendered text (form fields at their defaults) and close.
    func copySelection() {
        guard let snippet = model.selectedRow?.snippet else { return NSSound.beep() }
        let text = model.render(snippet, placeholders: false).text
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
        close()
    }

    /// ⌘E: open the snippet in the main window.
    func editSelection() {
        guard let id = model.selectedRow?.id else { return NSSound.beep() }
        close()
        onEdit?(id)
    }

    // MARK: - Keys

    private func handle(_ event: NSEvent) -> Bool {
        // Leave keys to an input method that is mid-composition (Return confirms a conversion).
        if let editor = panel?.firstResponder as? NSTextView, editor.hasMarkedText() { return false }
        let flags = event.modifierFlags.intersection([.command, .option, .control, .shift])
        switch Int(event.keyCode) {
        case kVK_UpArrow where flags.isEmpty, kVK_ANSI_P where flags == .control:
            model.moveSelection(by: -1)
        case kVK_DownArrow where flags.isEmpty, kVK_ANSI_N where flags == .control:
            model.moveSelection(by: 1)
        case kVK_PageUp where flags.isEmpty:
            model.moveSelection(by: -8)
        case kVK_PageDown where flags.isEmpty:
            model.moveSelection(by: 8)
        case kVK_Return where flags.isEmpty, kVK_ANSI_KeypadEnter where flags.isEmpty:
            insertSelection()
        case kVK_Return where flags == .command, kVK_ANSI_KeypadEnter where flags == .command:
            copySelection()
        case kVK_ANSI_E where flags == .command:
            editSelection()
        case kVK_Escape:
            close()
        case kVK_Tab:
            break // Keep focus in the search field.
        case kVK_ANSI_A where flags == .command:
            NSApp.sendAction(#selector(NSText.selectAll(_:)), to: nil, from: nil)
        case kVK_ANSI_C where flags == .command:
            NSApp.sendAction(#selector(NSText.copy(_:)), to: nil, from: nil)
        case kVK_ANSI_X where flags == .command:
            NSApp.sendAction(#selector(NSText.cut(_:)), to: nil, from: nil)
        case kVK_ANSI_V where flags == .command:
            // While another app is active the main menu may not route these, so send them directly.
            NSApp.sendAction(#selector(NSText.paste(_:)), to: nil, from: nil)
        default:
            return false
        }
        return true
    }

    // MARK: - Window

    func windowDidResignKey(_ notification: Notification) {
        close()
    }

    private func makePanel() -> SearchPalettePanel {
        let panel = SearchPalettePanel(size: Self.panelSize)
        panel.delegate = self
        panel.keyHandler = { [weak self] event in self?.handle(event) ?? false }

        let background = NSVisualEffectView()
        background.material = .popover
        background.blendingMode = .behindWindow
        background.state = .active
        let hosting = NSHostingView(rootView: SearchPaletteView(model: model, controller: self))
        hosting.sizingOptions = []
        hosting.translatesAutoresizingMaskIntoConstraints = false
        background.addSubview(hosting)
        NSLayoutConstraint.activate([
            hosting.leadingAnchor.constraint(equalTo: background.leadingAnchor),
            hosting.trailingAnchor.constraint(equalTo: background.trailingAnchor),
            hosting.topAnchor.constraint(equalTo: background.topAnchor),
            hosting.bottomAnchor.constraint(equalTo: background.bottomAnchor),
        ])
        panel.contentView = background
        self.panel = panel
        return panel
    }

    /// Centred horizontally in the upper third of the screen the mouse is on.
    private func position(_ panel: NSPanel) {
        let mouse = NSEvent.mouseLocation
        guard let screen = NSScreen.screens.first(where: { NSMouseInRect(mouse, $0.frame, false) }) ?? NSScreen.main else { return }
        let frame = screen.visibleFrame
        let size = panel.frame.size
        let top = frame.maxY - frame.height / 6
        panel.setFrameOrigin(NSPoint(
            x: (frame.midX - size.width / 2).rounded(),
            y: max(frame.minY, top - size.height).rounded()
        ))
    }
}
