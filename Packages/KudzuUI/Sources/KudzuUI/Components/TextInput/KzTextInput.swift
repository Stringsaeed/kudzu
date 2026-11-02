import SwiftUI

/// A single-line native text field in Kudzu's focusable well.
public struct KzTextInput: View {
    private let placeholder: String
    @Binding private var text: String
    private let mono: Bool
    private let focus: FocusState<Bool>.Binding?
    @FocusState private var localFocus: Bool
    @Environment(\.kzSnapshotRendering) private var snapshot
    @Environment(\.isEnabled) private var enabled
    /// Creates an input; optionally share focus with a parent command or form.
    public init(_ placeholder: String = "", text: Binding<String>, mono: Bool = false,
                focus: FocusState<Bool>.Binding? = nil) {
        self.placeholder = placeholder; _text = text; self.mono = mono; self.focus = focus
    }
    public var body: some View {
        Group {
            if snapshot {
                KzFieldWell(isFocused: false) { state in
                    KzFieldSnapshotText(text: text, placeholder: placeholder, mono: mono, enabled: state.isEnabled)
                        .padding(.horizontal, 10).frame(height: 34)
                }
            } else {
                TextField(placeholder, text: $text, prompt: Text(placeholder).foregroundStyle(enabled ? KzColor.inkMuted : KzDisabled.foreground))
                    .focused(focus ?? $localFocus)
                    .textFieldStyle(KzTextInputStyle(mono: mono, isFocused: (focus ?? $localFocus).wrappedValue))
            }
        }
    }
}

/// Custom styling for single-line native TextFields; pass their actual focus state.
@MainActor
public struct KzTextInputStyle: @preconcurrency TextFieldStyle {
    private let mono: Bool
    private let isFocused: Bool
    /// Creates the 34pt input style.
    public init(mono: Bool = false, isFocused: Bool = false) { self.mono = mono; self.isFocused = isFocused }
    public func _body(configuration: TextField<Self._Label>) -> some View {
        KzFieldWell(isFocused: isFocused) { state in
            configuration.textFieldStyle(.plain)
                .font((mono ? KzTextStyle.trigger : .body).font)
                .foregroundStyle(state.isEnabled ? KzColor.ink : KzDisabled.foreground)
                .padding(.horizontal, 10).frame(height: 34)
        }
    }
}

// Shared only by the controls' wells; no foundation changes required.
struct KzFieldWell<Content: View>: View {
    var isFocused: Bool
    @ViewBuilder var content: (KzControlState) -> Content
    var body: some View {
        let shape = RoundedRectangle(cornerRadius: KzRadius.md)
        KzInteractive(shape, stateLayer: false, focusRing: .edge, isFocused: isFocused) { state in
            content(state)
                .background(state.isFocused && state.isEnabled ? KzColor.surfaceRaised : KzColor.surfaceSunken, in: shape)
                .overlay(shape.strokeBorder(state.isHovered && !state.isFocused ? KzColor.line : .clear))
        }
    }
}

// ImageRenderer cannot draw AppKit-backed text fields; mirror only their text offscreen.
struct KzFieldSnapshotText: View {
    var text: String
    var placeholder: String
    var mono = false
    var enabled = true
    var body: some View {
        Text(text.isEmpty ? placeholder : text)
            .font((mono ? KzTextStyle.trigger : .body).font)
            .foregroundStyle(!enabled ? KzDisabled.foreground : text.isEmpty ? KzColor.inkMuted : KzColor.ink)
            .lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
    }
}
