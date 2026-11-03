import AppKit
import SwiftUI

/// Removable triggers and a native inline mono field.
///
/// Space and Return commit. Tab and leaving the field commit a valid draft and move on; an invalid
/// draft stays put, with the reason, until it is fixed or Escape clears just that uncommitted text.
/// Marked text is left for the input method.
public struct KzTriggerInput: View {
    @Binding private var triggers: [String]
    private let placeholder: String
    private let validate: (String) -> String?
    private let externalDraft: Binding<String>?
    private let externalError: Binding<String?>?
    private let onMove: ((Bool) -> Void)?
    @State private var storedDraft = ""
    @State private var storedError: String?
    @State private var focused = false
    @Environment(\.kzSnapshotRendering) private var snapshot

    /// `validate` returns a rejection reason or nil. Pass `draft` and `error` to keep an uncommitted
    /// entry when this view is recreated. `onMove` receives `true` for Tab and `false` for Shift-Tab
    /// after an empty or successfully committed draft; without it, Tab uses the key-view loop.
    public init(
        triggers: Binding<[String]>,
        placeholder: String = "Add trigger…",
        draft: Binding<String>? = nil,
        error: Binding<String?>? = nil,
        onMove: ((Bool) -> Void)? = nil,
        validate: @escaping (String) -> String? = { _ in nil }
    ) {
        _triggers = triggers
        self.placeholder = placeholder
        self.validate = validate
        externalDraft = draft
        externalError = error
        self.onMove = onMove
    }

    private var draft: Binding<String> { externalDraft ?? $storedDraft }
    private var error: Binding<String?> { externalError ?? $storedError }

    public var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            KzFieldWell(isFocused: focused) { state in
                TriggerFlow {
                    ForEach(Array(triggers.enumerated()), id: \.offset) { index, trigger in
                        KzTriggerChip(trigger, widthPolicy: .constrained) { triggers.remove(at: index) }
                            .help(trigger)
                            .environment(\.kzPreviewInteraction, nil)
                    }
                    Group {
                        if snapshot {
                            Text(draft.wrappedValue.isEmpty ? placeholder : draft.wrappedValue).font(KzTextStyle.trigger.font)
                        } else {
                            KzTriggerDraftField(
                                text: draft,
                                focused: $focused,
                                placeholder: placeholder,
                                commit: commit,
                                cancel: cancelDraft,
                                removeLast: { if !triggers.isEmpty { triggers.removeLast() } },
                                resign: { finish(cancelled: $0, isComposing: $1) },
                                passes: passes,
                                onMove: onMove
                            )
                        }
                    }
                    .foregroundStyle(state.isEnabled ? KzColor.inkMuted : KzDisabled.foreground)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .frame(minWidth: 100, minHeight: 22).padding(.horizontal, 4)
                }.padding(.horizontal, 6).padding(.vertical, 5).frame(minHeight: 34)
            }
            if let message = error.wrappedValue {
                Text(message).kzTextStyle(.caption).foregroundStyle(KzColor.danger).accessibilityLabel("Trigger rejected: \(message)")
            }
        }
    }

    /// Saves a valid draft. Returns false when there is nothing to save or the text was rejected.
    @discardableResult
    private func commit() -> Bool {
        let candidate = draft.wrappedValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !candidate.isEmpty else { return false }
        if let reason = Self.rejection(candidate, triggers: triggers, validate: validate) {
            error.wrappedValue = reason
            return false
        }
        triggers.append(candidate)
        draft.wrappedValue = ""
        error.wrappedValue = nil
        return true
    }

    private func cancelDraft() {
        draft.wrappedValue = ""
        error.wrappedValue = nil
    }

    private func passes(_ raw: String) -> Bool {
        let candidate = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !candidate.isEmpty else { return true }
        return Self.rejection(candidate, triggers: triggers, validate: validate) == nil
    }

    private func finish(cancelled: Bool, isComposing: Bool) {
        switch KzTriggerDraft.finish(
            text: draft.wrappedValue, isComposing: isComposing, cancelled: cancelled, isValid: passes(draft.wrappedValue)
        ) {
        case .keep: break
        case .clearError:
            if !draft.wrappedValue.isEmpty { draft.wrappedValue = "" }
            error.wrappedValue = nil
        case .commit: commit()
        case .reject: commit()
        }
    }

    static func rejection(_ candidate: String, triggers: [String], validate: (String) -> String?) -> String? {
        triggers.contains(candidate) ? "This trigger already exists." : validate(candidate)
    }
}

private struct TriggerFlow: Layout {
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        arrange(subviews, width: proposal.width ?? 400).size
    }
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let result = arrange(subviews, width: bounds.width)
        for (index, point) in result.points.enumerated() {
            let width = index == subviews.count - 1
                ? max(100, bounds.width - point.x)
                : min(bounds.width, subviews[index].sizeThatFits(.unspecified).width)
            subviews[index].place(at: CGPoint(x: bounds.minX + point.x, y: bounds.minY + point.y),
                                 proposal: ProposedViewSize(width: width, height: 22))
        }
    }
    private func arrange(_ subviews: Subviews, width: CGFloat) -> (size: CGSize, points: [CGPoint]) {
        var x: CGFloat = 0; var y: CGFloat = 0; var row: CGFloat = 0; var points: [CGPoint] = []
        for view in subviews {
            let natural = view.sizeThatFits(.unspecified)
            let size = CGSize(width: min(natural.width, width), height: natural.height)
            if x > 0 && x + size.width > width { x = 0; y += row + 6; row = 0 }
            points.append(CGPoint(x: x, y: y)); x += size.width + 6; row = max(row, size.height)
        }
        return (CGSize(width: width, height: y + row), points)
    }
}

struct KzTriggerDraftField: NSViewRepresentable {
    @Binding var text: String
    @Binding var focused: Bool
    let placeholder: String
    let commit: () -> Bool
    let cancel: () -> Void
    let removeLast: () -> Void
    /// Commit, keep, or clear the draft when the field resigns. The flags are cancelled and composing.
    let resign: (Bool, Bool) -> Void
    var passes: (String) -> Bool = { _ in true }
    var onMove: ((Bool) -> Void)?
    @Environment(\.isEnabled) private var enabled

    func makeCoordinator() -> Coordinator { Coordinator(self) }
    func makeNSView(context: Context) -> NSTextField {
        let field = NSTextField()
        field.isBordered = false; field.drawsBackground = false; field.focusRingType = .none
        field.font = KzTextStyle.trigger.nsFont; field.delegate = context.coordinator
        field.setAccessibilityLabel("Add trigger")
        field.setAccessibilityIdentifier("kudzu.trigger")
        field.setContentHuggingPriority(.defaultLow, for: .horizontal)
        return field
    }
    func updateNSView(_ field: NSTextField, context: Context) {
        context.coordinator.parent = self
        let composing = (field.currentEditor() as? NSTextView)?.hasMarkedText() == true
        if !composing, field.stringValue != text { field.stringValue = text }
        field.isEnabled = enabled
        field.textColor = enabled ? KzColorToken.ink.nsColor : KzColorToken.inkFaint.nsColor
        field.placeholderAttributedString = NSAttributedString(string: placeholder, attributes: KzTextStyle.trigger.attributes(color: KzColorToken.inkMuted.nsColor))
    }
    @MainActor final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: KzTriggerDraftField
        init(_ parent: KzTriggerDraftField) { self.parent = parent }
        func controlTextDidBeginEditing(_ obj: Notification) { parent.focused = true }
        func controlTextDidEndEditing(_ obj: Notification) {
            parent.focused = false
            let movement = (obj.userInfo?["NSTextMovement"] as? NSNumber)?.intValue
            let cancelled = movement == NSTextMovement.cancel.rawValue
            let composing = ((obj.object as? NSTextField)?.currentEditor() as? NSTextView)?.hasMarkedText() == true
            parent.resign(cancelled, composing)
            if let field = obj.object as? NSTextField, field.stringValue != parent.text { field.stringValue = parent.text }
        }
        func controlTextDidChange(_ obj: Notification) {
            guard let field = obj.object as? NSTextField else { return }
            parent.text = field.stringValue
            if field.stringValue.contains(" "), (field.currentEditor() as? NSTextView)?.hasMarkedText() != true {
                _ = parent.commit()
                field.stringValue = parent.text
            }
        }
        func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            let command: KzTriggerDraft.Command
            switch commandSelector {
            case #selector(NSResponder.insertNewline(_:)): command = .newline
            case #selector(NSResponder.insertTab(_:)): command = .tab
            case #selector(NSResponder.insertBacktab(_:)): command = .backtab
            case #selector(NSResponder.cancelOperation(_:)): command = .escape
            case #selector(NSResponder.deleteBackward(_:)): command = .deleteBackward
            default: return false
            }
            let result = KzTriggerDraft.handle(command, text: parent.text, isComposing: textView.hasMarkedText(), isValid: parent.passes(parent.text))
            switch result {
            case .passthrough:
                return false
            case .commit:
                _ = parent.commit()
                control.stringValue = parent.text
                return true
            case .reject:
                _ = parent.commit()
                control.stringValue = parent.text
                return true
            case .cancel:
                parent.cancel()
                control.stringValue = parent.text
                return true
            case .removeLast:
                parent.removeLast()
                return true
            case .move(let forward):
                guard let onMove = parent.onMove else { return false }
                onMove(forward)
                return true
            case .commitAndMove(let forward):
                let saved = parent.commit()
                control.stringValue = parent.text
                guard saved else { return true }
                guard let onMove = parent.onMove else { return false }
                onMove(forward)
                return true
            }
        }
    }
}
