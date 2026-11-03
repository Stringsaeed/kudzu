import AppKit
import Carbon
import KudzuCore
import KudzuUI
import SwiftUI

/// Shows, hides and drives the search palette on a `KzFloatingPanel`. Owned by `AppModel`.
@MainActor
final class SearchPaletteController: NSObject {
    static let panelSize = NSSize(width: 760, height: 460)

    let model: SearchPaletteModel
    /// Checks whether an explicit insert can proceed; returns an inline blocker message when it cannot.
    var preflightInsert: ((UUID, String?) -> String?)?
    /// Inserts a snippet into the frontmost app. Called after the palette has closed.
    var onInsert: ((UUID, pid_t?) async -> Void)?
    /// Opens the snippet in the main window.
    var onEdit: ((UUID) -> Void)?

    private(set) var panel: KzFloatingPanel?
    private(set) var targetAppPID: pid_t?
    private(set) var targetBundleID: String?
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
        let frontmost = NSWorkspace.shared.frontmostApplication
        targetAppPID = frontmost?.processIdentifier
        targetBundleID = frontmost?.bundleIdentifier
        let panel = panel ?? makePanel()
        model.isActive = true
        model.prepareForShow()
        panel.present(.centered(topFraction: 1.0 / 6))
        panel.contentView?.layoutSubtreeIfNeeded()
        if let field = model.searchField { panel.makeFirstResponder(field) }
    }

    func close() {
        guard let panel, panel.isVisible, !isClosing else { return }
        isClosing = true
        defer { isClosing = false }
        model.isActive = false
        model.actionError = nil
        panel.dismiss(.programmatic)
        // Order out immediately so the target app regains key focus before the 40 ms insert delay.
        panel.orderOut(nil)
    }

    // MARK: - Actions

    /// ↵: check preflight blockers first; if allowed, close so the previous app gets keyboard focus back, then insert.
    func insertSelection() {
        guard let id = model.selectedRow?.id else { return }
        if let blocker = preflightInsert?(id, targetBundleID) {
            model.actionError = blocker
            return
        }
        let expectedPID = targetAppPID
        close()
        Task { @MainActor in
            // Let the window server hand key focus back to the previous app before typing into it.
            try? await Task.sleep(for: .milliseconds(40))
            await onInsert?(id, expectedPID)
        }
    }

    /// ⌘↵: copy the rendered text (form fields at their defaults) and close.
    func copySelection() {
        guard let snippet = model.selectedRow?.snippet else { return }
        let text = model.render(snippet, placeholders: false).text
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
        close()
    }

    /// ⌘E: open the snippet in the main window.
    func editSelection() {
        guard let id = model.selectedRow?.id else { return }
        close()
        onEdit?(id)
    }

    // MARK: - Keys

    func handleKeyEvent(_ event: NSEvent) -> Bool {
        handle(event)
    }

    private func handle(_ event: NSEvent) -> Bool {
        // Leave keys to an input method that is mid-composition (Return confirms a conversion).
        if panel?.isComposingText == true || KzMenu.isOpen { return false }
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
            guard model.selectedRow != nil else { return true }
            insertSelection()
        case kVK_Return where flags == .command, kVK_ANSI_KeypadEnter where flags == .command:
            guard model.selectedRow != nil else { return true }
            copySelection()
        case kVK_ANSI_E where flags == .command:
            guard model.selectedRow != nil else { return true }
            editSelection()
        case kVK_Escape:
            close()
        case kVK_Tab:
            return false
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

    private func makePanel() -> KzFloatingPanel {
        let panel = KzFloatingPanel(sizing: .fixed(Self.panelSize), behavior: .transient) {
            SearchPaletteView(model: self.model, controller: self)
        }
        panel.keyHandler = { [weak self] event in self?.handle(event) ?? false }
        panel.onClose = { [weak self] _ in
            self?.model.isActive = false
        }
        self.panel = panel
        return panel
    }
}
