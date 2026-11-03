import AppKit
import SwiftUI

/// A list-pane search field, with an optional shortcut hint.
public struct KzSearchField: View {
    private let placeholder: String
    @Binding private var text: String
    private let shortcut: String?
    private let unboxed: Bool
    private let focus: FocusState<Bool>.Binding?
    private let selectAllToken: Int
    @FocusState private var localFocus: Bool
    @Environment(\.kzSnapshotRendering) private var snapshot
    @Environment(\.isEnabled) private var enabled

    /// Pass `focus` for ⌘F and increment `selectAllToken` to select the query; `unboxed` uses a flat system-font field.
    public init(_ placeholder: String = "Search snippets", text: Binding<String>, shortcut: String? = "⌘F",
                focus: FocusState<Bool>.Binding? = nil, selectAllToken: Int = 0, unboxed: Bool = false) {
        self.placeholder = placeholder
        self.unboxed = unboxed
        _text = text
        self.shortcut = shortcut
        self.focus = focus
        self.selectAllToken = selectAllToken
    }

    public var body: some View {
        let focusBinding = focus ?? $localFocus
        Group {
            if unboxed {
                input(focusBinding, enabled: enabled).overlay(alignment: .bottom) {
                    (focusBinding.wrappedValue ? KzColor.vineInk : KzColor.line).frame(height: 1)
                }
            } else {
                KzFieldWell(isFocused: focusBinding.wrappedValue) { state in input(focusBinding, enabled: state.isEnabled) }
            }
        }
    }

    private func input(_ focusBinding: FocusState<Bool>.Binding, enabled: Bool) -> some View {
        HStack(spacing: 8) {
            KzIcon(.search, size: 14).foregroundStyle(enabled ? KzColor.inkMuted : KzDisabled.foreground)
            Group {
                if snapshot {
                    KzFieldSnapshotText(text: text, placeholder: placeholder, enabled: enabled)
                } else {
                    KzSearchFieldBox(
                        text: $text,
                        placeholder: placeholder,
                        isFocused: focusBinding.wrappedValue,
                        selectAllToken: selectAllToken,
                        enabled: enabled,
                        unboxed: unboxed,
                        onFocus: { focusBinding.wrappedValue = $0 }
                    )
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            if let shortcut { KzKbd(shortcut).accessibilityHidden(true) }
        }.padding(.leading, 10).padding(.trailing, 6).frame(height: 30)
    }
}

/// The native field inside `KzSearchField`. Down, Tab, and Escape are routed by the window, not here.
struct KzSearchFieldBox: NSViewRepresentable {
    @Binding var text: String
    var placeholder: String
    var isFocused: Bool
    var selectAllToken: Int
    var enabled: Bool
    var unboxed = false
    var onFocus: (Bool) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeNSView(context: Context) -> NSTextField {
        let field = NSTextField()
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.font = KzTextStyle.body.nsFont
        field.delegate = context.coordinator
        field.lineBreakMode = .byTruncatingTail
        field.cell?.isScrollable = true
        field.cell?.sendsActionOnEndEditing = false
        field.maximumNumberOfLines = 1
        field.setAccessibilityIdentifier("kudzu.search")
        field.setAccessibilityLabel("Search snippets")
        field.setContentHuggingPriority(.defaultLow, for: .horizontal)
        field.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        return field
    }

    func updateNSView(_ field: NSTextField, context: Context) {
        context.coordinator.parent = self
        let composing = (field.currentEditor() as? NSTextView)?.hasMarkedText() == true
        if !composing, field.stringValue != text { field.stringValue = text }
        field.font = unboxed ? .systemFont(ofSize: 13) : KzTextStyle.body.nsFont
        field.isEnabled = enabled
        field.textColor = enabled ? KzColorToken.ink.nsColor : KzColorToken.inkFaint.nsColor
        field.placeholderAttributedString = NSAttributedString(
            string: placeholder,
            attributes: unboxed
                ? [.font: field.font ?? KzTextStyle.body.nsFont,
                   .foregroundColor: enabled ? KzColorToken.inkMuted.nsColor : KzColorToken.inkFaint.nsColor]
                : KzTextStyle.body.attributes(color: enabled ? KzColorToken.inkMuted.nsColor : KzColorToken.inkFaint.nsColor)
        )
        guard field.window != nil, !composing else { return }
        if isFocused, selectAllToken > 0, selectAllToken != context.coordinator.appliedSelectAll {
            context.coordinator.appliedSelectAll = selectAllToken
            // selectText both focuses and selects. makeFirstResponder alone can leave currentEditor nil
            // until the field editor is installed, which would drop the selection.
            field.selectText(nil)
            field.currentEditor()?.selectAll(nil)
            return
        }
        if isFocused, field.currentEditor() == nil {
            field.window?.makeFirstResponder(field)
        }
    }

    @MainActor final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: KzSearchFieldBox
        var appliedSelectAll = 0
        init(_ parent: KzSearchFieldBox) { self.parent = parent }

        func controlTextDidBeginEditing(_ obj: Notification) { parent.onFocus(true) }
        func controlTextDidEndEditing(_ obj: Notification) { parent.onFocus(false) }
        func controlTextDidChange(_ obj: Notification) {
            guard let field = obj.object as? NSTextField else { return }
            parent.text = field.stringValue
        }
    }
}
