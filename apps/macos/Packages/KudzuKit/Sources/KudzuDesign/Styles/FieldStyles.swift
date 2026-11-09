// Text fields (`input.tsx`, `textarea.tsx`, `input-group.tsx`) as native `TextField` /
// `TextEditor` with Kudzu's well: `bg-foreground/[0.025]` (dark `bg-black/15`) sunk with the well
// depth, `rounded-lg`, 32 pt tall, `px-2.5`, 14 pt text, muted placeholder. Focused, the well clears
// (`focus-visible:bg-transparent shadow-none`) and the system focus ring shows. Invalid fields get the
// destructive border.

import SwiftUI

extension View {
    /// `Input`: apply to a `TextField` (`TextField("Name", text: $name).kzInput()`). (A custom
    /// `TextFieldStyle` can't host main-actor focus state under Swift 6, so this is a modifier.)
    /// `height` follows a `className="h-9"` override; the default is the input's `h-8`.
    public func kzInput(invalid: Bool = false, height: CGFloat = 32) -> some View {
        self
            .textFieldStyle(.plain)
            .font(.system(size: 14))
            .padding(.horizontal, 10)
            .frame(height: height)
            .kzWell(invalid: invalid)
    }

    /// The input well behind a field or group: rounded-lg fill + well depth, cleared while focused.
    public func kzWell(invalid: Bool = false, radius: CGFloat = Radius.lg) -> some View {
        modifier(Well(invalid: invalid, radius: radius, externalFocus: nil))
    }

    /// `Textarea`: a `TextEditor` in the well, `min-h-16` (64), `px-2.5 py-2`. `minHeight` covers a
    /// `min-h-20` override (the chip editor's options field).
    public func kzTextarea(invalid: Bool = false, minHeight: CGFloat = 64) -> some View {
        self
            .font(.system(size: 14))
            .scrollContentBackground(.hidden)
            .padding(.horizontal, 6)
            .padding(.vertical, 8)
            .frame(minHeight: minHeight)
            .kzWell(invalid: invalid)
    }
}

struct Well: ViewModifier {
    var invalid: Bool
    var radius: CGFloat
    /// The field's focus when the well wraps more than the field (input groups).
    var externalFocus: Bool?
    @FocusState private var ownFocus: Bool
    @Environment(\.colorScheme) private var scheme
    @Environment(\.isEnabled) private var isEnabled

    func body(content: Content) -> some View {
        let t = KZTheme(scheme)
        let shape = RoundedRectangle(cornerRadius: radius)
        let focused = externalFocus ?? ownFocus
        let fill: Color = !isEnabled ? t[.input, alpha: t.dark ? 0.8 : 0.5] : (t.dark ? KZTheme.black(0.15) : t[.foreground, alpha: 0.025])
        content
            .modifier(OwnFocus(binding: $ownFocus, enabled: externalFocus == nil))
            .background {
                if focused {
                    shape.fill(Color.clear)
                } else {
                    Color.clear.depth(.well, fill: fill, in: shape)
                }
            }
            .overlay {
                if invalid { shape.strokeBorder(t[.destructive, alpha: t.dark ? 0.5 : 1], lineWidth: 1) }
            }
            .opacity(isEnabled ? 1 : 0.5)
            .themeAwareAnimation(.kz.colors, value: focused)
    }
}

private struct OwnFocus: ViewModifier {
    var binding: FocusState<Bool>.Binding
    var enabled: Bool
    func body(content: Content) -> some View {
        if enabled { content.focused(binding) } else { content }
    }
}

/// `InputGroup`: a field with inline addons (icons, text, keycaps, small buttons) sharing one well.
/// `InputGroup { Icon.search.icon() } field: { TextField("Search", text: $q) } trailing: { Shortcut("mod+k") }`.
public struct InputGroup<Leading: View, Field: View, Trailing: View>: View {
    var leading: Leading
    var field: Field
    var trailing: Trailing
    var invalid: Bool
    @FocusState private var fieldFocused: Bool

    /// Creates InputGroup with the supplied content and styling.
    public init(invalid: Bool = false, @ViewBuilder leading: () -> Leading, @ViewBuilder field: () -> Field, @ViewBuilder trailing: () -> Trailing) {
        self.leading = leading()
        self.field = field()
        self.trailing = trailing()
        self.invalid = invalid
    }

    /// The composed native view.
    public var body: some View {
        HStack(spacing: 8) {
            // `inline-start`: `pl-2`, muted 14 pt medium, 16 pt icons.
            leading.mutedForeground().font(.system(size: 14, weight: .medium))
            field.textFieldStyle(.plain).font(.system(size: 14)).focused($fieldFocused)
            trailing.mutedForeground().font(.system(size: 14, weight: .medium))
        }
        .padding(.horizontal, 8)
        .frame(height: 32)
        .modifier(Well(invalid: invalid, radius: Radius.lg, externalFocus: fieldFocused))
    }
}

extension InputGroup where Trailing == EmptyView {
    /// Creates InputGroup with the supplied content and styling.
    public init(invalid: Bool = false, @ViewBuilder leading: () -> Leading, @ViewBuilder field: () -> Field) {
        self.init(invalid: invalid, leading: leading, field: field, trailing: { EmptyView() })
    }
}

/// shadcn `Label`: 14 pt medium, `leading-none`, 8 pt gap to its control.
public struct FieldLabel<Content: View>: View {
    var title: String
    var content: Content
    /// Creates FieldLabel with the supplied content and styling.
    public init(_ title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }
    /// The composed native view.
    public var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(.system(size: 14, weight: .medium))
            content
        }
    }
}
