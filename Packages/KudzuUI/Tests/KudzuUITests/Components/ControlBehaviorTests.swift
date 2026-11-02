@testable import KudzuUI
import Testing
import SwiftUI
import AppKit

@MainActor
@Suite struct ControlBehaviorTests {
    @Test func stepperClampsAtInclusiveBoundsAndAvoidsOverflow() {
        #expect(KzStepper.adjusted(9, direction: 1, step: 5, min: 0, max: 10) == 10)
        #expect(KzStepper.adjusted(1, direction: -1, step: 5, min: 0, max: 10) == 0)
        #expect(KzStepper.adjusted(Int.max, direction: 1, step: 1, min: .min, max: .max) == .max)
        #expect(KzStepper.adjusted(Int.min, direction: -1, step: 1, min: .min, max: .max) == .min)
    }
    @Test func triggerValidationPreservesExactTextAndReportsWhy() {
        #expect(KzTriggerInput.rejection(":an", triggers: [":an"]) { _ in nil } == "This trigger already exists.")
        #expect(KzTriggerInput.rejection(":AN", triggers: [":an"]) { _ in nil } == nil)
        #expect(KzTriggerInput.rejection(":long", triggers: []) { $0.count > 3 ? "Too long" : nil } == "Too long")
    }
    @Test func triggerNativeCommandsCommitRemoveAndLeaveTabAlone() {
        var draft = ":an"
        var triggers: [String] = []
        let field = KzTriggerDraftField(
            text: Binding(get: { draft }, set: { draft = $0 }),
            focused: .constant(false), placeholder: "Add trigger…",
            commit: { triggers.append(draft.trimmingCharacters(in: .whitespaces)); draft = "" },
            removeLast: { if !triggers.isEmpty { triggers.removeLast() } }
        )
        let delegate = KzTriggerDraftField.Coordinator(field)
        let native = NSTextField()
        let editor = NSTextView()
        #expect(delegate.control(native, textView: editor, doCommandBy: #selector(NSResponder.insertNewline(_:))))
        #expect(triggers == [":an"])
        #expect(draft == "")
        #expect(delegate.control(native, textView: editor, doCommandBy: #selector(NSResponder.deleteBackward(_:))))
        #expect(triggers.isEmpty)
        #expect(!delegate.control(native, textView: editor, doCommandBy: #selector(NSResponder.insertTab(_:))))
        draft = ":sig"
        #expect(!delegate.control(native, textView: editor, doCommandBy: #selector(NSResponder.deleteBackward(_:))))
        #expect(draft == ":sig")
        native.stringValue = ":sig "
        delegate.controlTextDidChange(Notification(name: NSControl.textDidChangeNotification, object: native))
        #expect(triggers == [":sig"])
        #expect(native.stringValue == "")
    }

}
