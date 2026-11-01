import AppKit
import SwiftUI

/// Removable triggers and a native inline mono field; Space/Return commit and Tab moves on.
public struct KzTriggerInput: View {
    @Binding private var triggers: [String]
    private let placeholder: String
    private let validate: (String) -> String?
    @State private var draft = ""
    @State private var error: String?
    @State private var focused = false
    @Environment(\.kzSnapshotRendering) private var snapshot
    /// `validate` returns a rejection reason or nil; rejected drafts remain editable and the reason is shown below.
    public init(triggers: Binding<[String]>, placeholder: String = "Add trigger…", validate: @escaping (String) -> String? = { _ in nil }) {
        _triggers = triggers; self.placeholder = placeholder; self.validate = validate
    }
    public var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            KzFieldWell(isFocused: focused) { state in
                TriggerFlow {
                    ForEach(Array(triggers.enumerated()), id: \.offset) { index, trigger in
                        KzTriggerChip(trigger) { triggers.remove(at: index) }
                            .environment(\.kzPreviewInteraction, nil)
                    }
                    Group {
                        if snapshot {
                            Text(draft.isEmpty ? placeholder : draft).font(KzTextStyle.trigger.font)
                        } else {
                            KzTriggerDraftField(text: $draft, focused: $focused, placeholder: placeholder, commit: commit) {
                                if !triggers.isEmpty { triggers.removeLast() }
                            }
                        }
                    }
                    .foregroundStyle(state.isEnabled ? KzColor.inkMuted : KzDisabled.foreground)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .frame(minWidth: 100, minHeight: 22).padding(.horizontal, 4)
                }.padding(.horizontal, 6).padding(.vertical, 5).frame(minHeight: 34)
            }
            if let error { Text(error).kzTextStyle(.caption).foregroundStyle(KzColor.danger).accessibilityLabel("Trigger rejected: \(error)") }
        }
    }
    private func commit() {
        let candidate = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !candidate.isEmpty else { return }
        if let reason = Self.rejection(candidate, triggers: triggers, validate: validate) { error = reason; return }
        triggers.append(candidate); draft = ""; error = nil
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
            subviews[index].place(at: CGPoint(x: bounds.minX + point.x, y: bounds.minY + point.y), proposal: index == subviews.count - 1 ? ProposedViewSize(width: max(100, bounds.width - point.x), height: 22) : .unspecified)
        }
    }
    private func arrange(_ subviews: Subviews, width: CGFloat) -> (size: CGSize, points: [CGPoint]) {
        var x: CGFloat = 0; var y: CGFloat = 0; var row: CGFloat = 0; var points: [CGPoint] = []
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
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
    let commit: () -> Void
    let removeLast: () -> Void
    @Environment(\.isEnabled) private var enabled
    func makeCoordinator() -> Coordinator { Coordinator(self) }
    func makeNSView(context: Context) -> NSTextField {
        let field = NSTextField()
        field.isBordered = false; field.drawsBackground = false; field.focusRingType = .none
        field.font = KzTextStyle.trigger.nsFont; field.delegate = context.coordinator
        field.setAccessibilityLabel("Add trigger")
        field.setContentHuggingPriority(.defaultLow, for: .horizontal)
        return field
    }
    func updateNSView(_ field: NSTextField, context: Context) {
        context.coordinator.parent = self
        if field.stringValue != text { field.stringValue = text }
        field.isEnabled = enabled
        field.textColor = enabled ? KzColorToken.ink.nsColor : KzColorToken.inkFaint.nsColor
        field.placeholderAttributedString = NSAttributedString(string: placeholder, attributes: KzTextStyle.trigger.attributes(color: KzColorToken.inkMuted.nsColor))
    }
    @MainActor final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: KzTriggerDraftField
        init(_ parent: KzTriggerDraftField) { self.parent = parent }
        func controlTextDidBeginEditing(_ obj: Notification) { parent.focused = true }
        func controlTextDidEndEditing(_ obj: Notification) { parent.focused = false }
        func controlTextDidChange(_ obj: Notification) {
            guard let field = obj.object as? NSTextField else { return }
            parent.text = field.stringValue
            if field.stringValue.contains(" "), (field.currentEditor() as? NSTextView)?.hasMarkedText() != true {
                parent.commit()
                field.stringValue = parent.text
            }
        }
        func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.insertNewline(_:)), !textView.hasMarkedText() {
                parent.commit(); control.stringValue = parent.text; return true
            }
            if commandSelector == #selector(NSResponder.deleteBackward(_:)), parent.text.isEmpty {
                parent.removeLast(); return true
            }
            return false
        }
    }
}
