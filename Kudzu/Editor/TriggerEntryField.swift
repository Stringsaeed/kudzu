import AppKit
import SwiftUI

/// Borderless text field that reports ↵ and ⌫-on-empty, which SwiftUI's `TextField` can't.
struct TriggerEntryField: NSViewRepresentable {
    @Binding var text: String
    /// Increment to move keyboard focus into the field.
    var focusToken: Int
    var placeholder: String
    var onSubmit: () -> Void
    var onDeleteWhenEmpty: () -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeNSView(context: Context) -> NSTextField {
        let field = NSTextField()
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.font = .monospacedSystemFont(ofSize: NSFont.systemFontSize, weight: .regular)
        field.placeholderString = placeholder
        field.cell?.usesSingleLineMode = true
        field.cell?.wraps = false
        field.cell?.isScrollable = true
        field.delegate = context.coordinator
        context.coordinator.lastFocusToken = focusToken
        return field
    }

    func updateNSView(_ field: NSTextField, context: Context) {
        context.coordinator.parent = self
        if field.stringValue != text { field.stringValue = text }
        field.placeholderString = placeholder
        if context.coordinator.lastFocusToken != focusToken {
            context.coordinator.lastFocusToken = focusToken
            DispatchQueue.main.async { field.window?.makeFirstResponder(field) }
        }
    }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView: NSTextField, context: Context) -> CGSize? {
        CGSize(width: max(proposal.width ?? 150, 60), height: max(nsView.intrinsicContentSize.height, 22))
    }

    @MainActor
    final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: TriggerEntryField
        var lastFocusToken = 0

        init(_ parent: TriggerEntryField) { self.parent = parent }

        func controlTextDidChange(_ notification: Notification) {
            guard let field = notification.object as? NSTextField else { return }
            parent.text = field.stringValue
        }

        func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
            switch selector {
            case #selector(NSResponder.insertNewline(_:)):
                parent.onSubmit()
                return true
            case #selector(NSResponder.deleteBackward(_:)) where textView.string.isEmpty:
                parent.onDeleteWhenEmpty()
                return true
            case #selector(NSResponder.insertTab(_:)) where !textView.string.isEmpty:
                // Commit what was typed, then let Tab move on.
                parent.onSubmit()
                return false
            default:
                return false
            }
        }
    }
}
