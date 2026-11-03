import AppKit
import KudzuUI
import SwiftUI

/// Installs the main window's keyboard router without changing window geometry or policy.
struct MainWindowKeyMonitor: NSViewRepresentable {
    var keyContext: @MainActor (NSWindow) -> KzLibraryKeyContext
    var perform: @MainActor (KzLibraryAction) -> Void

    func makeNSView(context: Context) -> KeyMonitorView {
        let view = KeyMonitorView()
        view.keyContext = keyContext
        view.perform = perform
        return view
    }

    func updateNSView(_ view: KeyMonitorView, context: Context) {
        view.keyContext = keyContext
        view.perform = perform
    }

    static func dismantleNSView(_ view: KeyMonitorView, coordinator: ()) {
        view.teardown()
    }
}

/// Routes only this window's library keys; settings owns its own focus session.
final class KeyMonitorView: NSView {
    var keyContext: @MainActor (NSWindow) -> KzLibraryKeyContext = { _ in KzLibraryKeyContext(control: .other) }
    var perform: @MainActor (KzLibraryAction) -> Void = { _ in }
    private var monitor: Any?

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        teardown()
        guard window != nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            let handled = MainActor.assumeIsolated { () -> Bool in
                guard let self, let window = self.window, event.window === window else { return false }
                let context = self.keyContext(window)
                guard let key = KzLibraryKeyParser.key(
                    code: event.keyCode, shift: event.modifierFlags.contains(.shift),
                    command: event.modifierFlags.contains(.command), option: event.modifierFlags.contains(.option),
                    control: event.modifierFlags.contains(.control)
                ) else { return false }
                let action = KzLibraryKeyRouter.route(key, in: context)
                guard action != .passthrough else { return false }
                self.perform(action)
                return true
            }
            return handled ? nil : event
        }
    }

    func teardown() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }

    isolated deinit { if let monitor { NSEvent.removeMonitor(monitor) } }
}

/// Which library control owns the main window's first responder.
@MainActor
enum MainWindowFocus {
    static func control(in window: NSWindow) -> KzFocusedControl {
        let responder = window.firstResponder
        if let textView = responder as? NSTextView, textView is ChipTextView {
            return .editorContent
        }

        var fieldIdentifier: String?
        var fieldLabel: String?
        var view: NSView?
        if let textView = responder as? NSTextView, let field = textView.delegate as? NSTextField {
            let fieldID = field.accessibilityIdentifier()
            fieldIdentifier = fieldID.isEmpty ? nil : fieldID
            fieldLabel = field.accessibilityLabel()
            view = field
        } else {
            view = responder as? NSView
        }

        var identifier = fieldIdentifier
        var snippetTable = false
        var current = view
        while let candidate = current {
            let id = candidate.accessibilityIdentifier()
            if !id.isEmpty {
                if id == "kudzu.results" { snippetTable = true }
                if identifier == nil, id.hasPrefix("kudzu.") { identifier = id }
            }
            current = candidate.superview
        }
        if identifier == nil {
            switch fieldLabel {
            case "Search snippets": identifier = "kudzu.search"
            case "Add trigger": identifier = "kudzu.trigger"
            case "Name": identifier = "kudzu.editor.name"
            case "Group name": identifier = "kudzu.rename"
            default: break
            }
        }
        return KzFocusIdentity.control(identifier: identifier, isTable: snippetTable, isChipText: false)
    }

    /// Pending trigger text, preferring the live field editor over the stored draft.
    static func triggerDraftPending(control: KzFocusedControl, window: NSWindow, stored: String?) -> Bool {
        guard control == .editorTrigger else { return false }
        let live = (window.firstResponder as? NSTextView)?.string ?? stored ?? ""
        return !live.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}

/// Marks the snippet list's table so Delete and arrows are not taken from the other tables in the window.
struct ResultsTableTag: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { NSView(frame: .zero) }

    func updateNSView(_ view: NSView, context: Context) {
        DispatchQueue.main.async { [weak view] in
            guard let view else { return }
            Self.markTable(near: view)
        }
    }

    private static func markTable(near view: NSView) {
        var current = view.superview
        while let candidate = current {
            if let table = firstTable(in: candidate) {
                if table.accessibilityIdentifier() != "kudzu.results" {
                    table.setAccessibilityIdentifier("kudzu.results")
                }
                return
            }
            current = candidate.superview
        }
    }

    private static func firstTable(in view: NSView) -> NSTableView? {
        if let table = view as? NSTableView { return table }
        for child in view.subviews {
            if let table = firstTable(in: child) { return table }
        }
        return nil
    }
}
