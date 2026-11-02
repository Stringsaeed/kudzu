import AppKit

/// A readable surface that always fits inside the current content rectangle.
enum SettingsModalLayout {
    static func frame(in size: CGSize) -> CGRect {
        let margin = min(24, max(0, min(size.width, size.height) / 12))
        let width = min(960, max(0, size.width - 2 * margin))
        let height = min(760, max(0, size.height - 2 * margin))
        return CGRect(x: (size.width - width) / 2, y: (size.height - height) / 2,
                      width: width, height: height)
    }
}

/// Keeps keyboard navigation inside a same-window settings host and remembers the editor's caret.
@MainActor
final class SettingsModalFocusSession {
    private weak var host: NSView?
    private weak var window: NSWindow?
    private weak var previousResponder: NSView?
    private var selection: NSRange?
    private var monitor: Any?
    var close: () -> Void = {}
    var keyboardCaptured = false

    static func hasNestedPresentation(in window: NSWindow) -> Bool {
        window.attachedSheet != nil || (window.childWindows ?? []).contains { $0.isVisible }
    }

    static func canHandleKeys(in window: NSWindow, keyWindow: NSWindow?, hasNestedPresentation: Bool,
                              keyboardCaptured: Bool = false) -> Bool {
        keyWindow === window && !hasNestedPresentation && !keyboardCaptured
    }

    static func blocksLibraryShortcut(characters: String?, modifiers: NSEvent.ModifierFlags) -> Bool {
        modifiers.intersection([.command, .control, .option, .shift]) == .command
            && ["f", "n", "s", "d"].contains(characters?.lowercased())
    }

    func begin(in host: NSView) {
        guard monitor == nil, let window = host.window else { return }
        self.host = host
        self.window = window
        let responder = window.firstResponder
        if let editor = responder as? NSTextView {
            selection = editor.selectedRange()
            previousResponder = editor.isFieldEditor ? editor.delegate as? NSView : editor
        } else {
            previousResponder = responder as? NSView
        }
        window.makeFirstResponder(host)
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self else { return event }
            return self.handle(event)
        }
    }

    func requestClose() {
        guard let window = host?.window,
              Self.canHandleKeys(in: window, keyWindow: NSApp.keyWindow,
                                 hasNestedPresentation: Self.hasNestedPresentation(in: window)) else { return }
        close()
    }

    private func handle(_ event: NSEvent) -> NSEvent? {
        guard let host, let window = host.window, event.window === window,
              Self.canHandleKeys(in: window, keyWindow: NSApp.keyWindow,
                                 hasNestedPresentation: Self.hasNestedPresentation(in: window),
                                 keyboardCaptured: keyboardCaptured) else { return event }
        let isComposing = (window.firstResponder as? NSTextView)?.hasMarkedText() == true
        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if event.keyCode == 53, modifiers.isDisjoint(with: [.command, .control, .option]) {
            // Escape belongs to input-method composition until marked text is resolved.
            if isComposing { return event }
            requestClose()
            return nil
        }
        if modifiers.intersection([.command, .control, .option, .shift]) == .command,
           event.charactersIgnoringModifiers == "[" {
            requestClose()
            return nil
        }
        // Library Find/New and their hidden view shortcuts must not escape the modal.
        if Self.blocksLibraryShortcut(characters: event.charactersIgnoringModifiers, modifiers: modifiers) {
            return nil
        }
        if event.keyCode == 48, modifiers.isDisjoint(with: [.command, .control, .option]) {
            if isComposing { return event }
            let current = Self.focusView(window.firstResponder) ?? host
            let next = Self.nextView(from: current, in: host, backwards: modifiers.contains(.shift))
            window.makeFirstResponder(next ?? host)
            return nil
        }
        if let focused = Self.focusView(window.firstResponder), !focused.isDescendant(of: host) {
            // A background programmatic focus request must not send typing to the editor.
            window.makeFirstResponder(host)
        }
        return event
    }

    private static func focusView(_ responder: NSResponder?) -> NSView? {
        if let editor = responder as? NSTextView, editor.isFieldEditor {
            return editor.delegate as? NSView
        }
        return responder as? NSView
    }

    static func nextView(from start: NSView, in host: NSView, backwards: Bool) -> NSView? {
        var visited = Set<ObjectIdentifier>()
        var candidate = backwards ? start.previousValidKeyView : start.nextValidKeyView
        while let view = candidate, visited.insert(ObjectIdentifier(view)).inserted {
            if view.isDescendant(of: host) { return view }
            candidate = backwards ? view.previousValidKeyView : view.nextValidKeyView
        }
        return nil
    }

    func end() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        defer { host = nil; window = nil; previousResponder = nil; selection = nil }
        guard let window, window.isKeyWindow,
              !Self.hasNestedPresentation(in: window), let previousResponder,
              previousResponder.window === window, !previousResponder.isHiddenOrHasHiddenAncestor else { return }
        guard window.makeFirstResponder(previousResponder), let selection,
              let editor = window.firstResponder as? NSTextView else { return }
        let length = (editor.string as NSString).length
        let location = min(selection.location, length)
        editor.setSelectedRange(NSRange(location: location, length: min(selection.length, length - location)))
    }
}
