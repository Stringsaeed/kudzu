// `TriggerInput` (`trigger-input.tsx`): a snippet's triggers as mono chips in a field well, with a
// text field for the next one. Typing rules live in `TriggerEntry`. Chips that collide with
// another snippet turn amber; each has a remove button; hovering the well shows a faint +.

import KudzuCore
import KudzuDesign
import SwiftUI

/// The focusable fields of the editor header (name → triggers on Enter).
enum EditorField: Hashable { case name, trigger }

struct TriggerInput: View {
    var triggers: [String]
    var onChange: ([String]) -> Void
    var caseSensitive: Bool
    var prefix: String
    /// Triggers (lowercased) that collide with another snippet.
    var conflicted: Set<String>
    var focus: FocusState<EditorField?>.Binding

    @State private var entry = TriggerEntry()
    @State private var hovered = false
    @Environment(\.colorScheme) private var scheme

    private var isFocused: Bool { focus.wrappedValue == .trigger }

    var body: some View {
        let t = KZTheme(scheme)
        let shape = RoundedRectangle(cornerRadius: Radius.lg)
        VStack(alignment: .leading, spacing: 6) {
            FlowLayout(spacing: 6, lineSpacing: 6) {
                ForEach(Array(triggers.enumerated()), id: \.offset) { index, trigger in
                    TriggerToken(trigger: trigger, conflicted: conflicted.contains(trigger.lowercased())) {
                        withAnimation(.kz.springPop()) { onChange(triggers.enumerated().filter { $0.offset != index }.map(\.element)) }
                    }
                    .id("\(trigger)-\(index)")
                    .transition(KZTransition.pop)
                }
                field(t)
            }
            .overlay(alignment: .trailing) {
                if entry.draft.isEmpty, !triggers.isEmpty {
                    Icon.plus.icon(14)
                        .foregroundStyle(t[.faint])
                        .opacity(hovered ? 1 : 0)
                        .animation(.kz.hover, value: hovered)
                        .padding(.trailing, 4)
                        .allowsHitTesting(false)
                }
            }
            .padding(.horizontal, 6)
            .padding(.vertical, 4)
            .frame(minHeight: 36)
            .background {
                if isFocused { shape.fill(Color.clear) } else {
                    Color.clear.depth(.well, fill: t.dark ? KZTheme.black(0.15) : t[.foreground, alpha: 0.025], in: shape)
                }
            }
            .overlay {
                if entry.error != nil {
                    shape.strokeBorder(t[.destructive, alpha: 0.6], lineWidth: isFocused ? 2 : 1)
                } else if isFocused {
                    shape.strokeBorder(t[.ring], lineWidth: 2)
                }
            }
            .animation(.kz.colors, value: isFocused)
            .contentShape(shape)
            .onTapGesture { focus.wrappedValue = .trigger }
            .onHover { hovered = $0 }

            if let error = entry.error {
                Text(error)
                    .font(.system(size: 12))
                    .foregroundStyle(t[.destructive])
                    .accessibilityAddTraits(.updatesFrequently)
            }
        }
        .onChange(of: focus.wrappedValue) { old, new in
            if new == .trigger, old != .trigger { entry.focused(triggers: triggers, prefix: prefix) }
            if old == .trigger, new != .trigger, let next = entry.blurred(triggers: triggers, prefix: prefix, caseSensitive: caseSensitive) {
                onChange(next)
            }
        }
    }

    private func field(_ t: KZTheme) -> some View {
        let text = Binding(
            get: { entry.draft },
            set: { value in
                if let next = entry.typed(value, triggers: triggers, caseSensitive: caseSensitive) { onChange(next) }
            }
        )
        return TextField("Add trigger", text: text, prompt: Text(TriggerEntry.placeholder(triggers: triggers, prefix: prefix)).font(.system(size: 12.5)))
            .labelsHidden()
            .textFieldStyle(.plain)
            .font(KZFonts.mono(size: 12.5))
            .autocorrectionDisabled()
            .focused(focus, equals: .trigger)
            .padding(.horizontal, 4)
            .frame(minWidth: 96, minHeight: 24)
            .layoutPriority(1)
            .accessibilityLabel("Add trigger")
            .onSubmit { commitIfReady() }
            .onKeyPress(.tab) { entry.canCommit(prefix: prefix) && commitIfReady() ? .handled : .ignored }
            .onKeyPress(.delete) {
                guard let next = entry.backspace(triggers: triggers) else { return .ignored }
                onChange(next)
                return .handled
            }
            .onKeyPress(.escape) { entry.escape() ? .handled : .ignored }
    }

    /// Enter (and Tab with something typed) adds the draft.
    @discardableResult
    private func commitIfReady() -> Bool {
        guard entry.canCommit(prefix: prefix) else { return false }
        if let next = entry.commit(triggers: triggers, caseSensitive: caseSensitive) { onChange(next) }
        return true
    }
}

/// One trigger: `h-6 rounded-md pl-2 pr-1 font-mono text-[12px]`, iris-soft or amber when it
/// conflicts, with a 16 pt remove button that brightens on hover.
private struct TriggerToken: View {
    var trigger: String
    var conflicted: Bool
    var onRemove: () -> Void
    @State private var hovered = false
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let t = KZTheme(scheme)
        HStack(spacing: 4) {
            Text(verbatim: trigger).font(KZFonts.mono(size: 12)).lineLimit(1)
            Button(action: onRemove) {
                Icon.x.icon(12)
                    .frame(width: 16, height: 16)
                    .background(hovered ? t[.foreground, alpha: 0.1] : .clear, in: RoundedRectangle(cornerRadius: 4))
                    .opacity(hovered ? 1 : 0.5)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .onHover { hovered = $0 }
            .accessibilityLabel("Remove trigger \(trigger)")
        }
        .foregroundStyle(conflicted ? t[.warning] : t[.irisForeground])
        .padding(.leading, 8)
        .padding(.trailing, 4)
        .frame(height: 24)
        .background(conflicted ? t[.warning, alpha: 0.15] : t[.irisSoft], in: RoundedRectangle(cornerRadius: Radius.md))
        .help(conflicted ? "Conflicts with another snippet" : "")
        .pop()
    }
}
