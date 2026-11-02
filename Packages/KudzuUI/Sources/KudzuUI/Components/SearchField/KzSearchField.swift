import SwiftUI

/// A list-pane search field, with an optional shortcut hint.
public struct KzSearchField: View {
    private let placeholder: String
    @Binding private var text: String
    private let shortcut: String?
    private let focus: FocusState<Bool>.Binding?
    @FocusState private var localFocus: Bool
    @Environment(\.kzSnapshotRendering) private var snapshot
    @Environment(\.isEnabled) private var enabled
    /// Pass `focus` and set it from a Commands button with `.keyboardShortcut("f")` to wire ⌘F.
    public init(_ placeholder: String = "Search snippets", text: Binding<String>, shortcut: String? = "⌘F",
                focus: FocusState<Bool>.Binding? = nil) {
        self.placeholder = placeholder; _text = text; self.shortcut = shortcut; self.focus = focus
    }
    public var body: some View {
        KzFieldWell(isFocused: (focus ?? $localFocus).wrappedValue) { state in
            HStack(spacing: 8) {
                KzIcon(.search, size: 14).foregroundStyle(state.isEnabled ? KzColor.inkMuted : KzDisabled.foreground)
                Group {
                    if snapshot {
                        KzFieldSnapshotText(text: text, placeholder: placeholder, enabled: state.isEnabled)
                    } else {
                        TextField(placeholder, text: $text, prompt: Text(placeholder).foregroundStyle(enabled ? KzColor.inkMuted : KzDisabled.foreground))
                            .textFieldStyle(.plain).font(KzTextStyle.body.font)
                            .foregroundStyle(state.isEnabled ? KzColor.ink : KzDisabled.foreground)
                            .focused(focus ?? $localFocus)
                    }
                }
                if let shortcut { KzKbd(shortcut).accessibilityHidden(true) }
            }.padding(.leading, 10).padding(.trailing, 6).frame(height: 30)
        }
    }
}
