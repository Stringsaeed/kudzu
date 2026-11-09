// Conventions for native controls, tinted with the tokens. The native look is accepted where the
// platform dictates it (switch, checkbox, radio, menu, segmented control):
//
// | web (`components/ui`)            | native                                                        |
// |----------------------------------|---------------------------------------------------------------|
// | Switch (`data-checked:bg-iris`)  | `Toggle` + `.kzSwitch(size:)` → `.switch`, iris tint, small/mini |
// | Checkbox                         | `Toggle` + `.kzCheckbox()` → `.checkbox`, primary tint        |
// | RadioGroup                       | `Picker` + `.kzRadioGroup()` → `.radioGroup`                  |
// | Select (trigger + content)       | `Picker` + `.kzSelect(size:)` → `.menu` popup button          |
// | Tabs / ToggleGroup / Segmented   | `Segmented` (Kudzu's well and sliding pill, not the accent)   |
// | Toggle (pressed state)           | `Toggle` + `.toggleStyle(.button)` + `.kzButton`              |
// | DropdownMenu / ContextMenu       | `Menu` + `.kzMenuTrigger(...)`, `.contextMenu`; items are     |
// |                                  | `Button`/`Toggle`/`Picker(.inline)`/`Menu` (sub-menu)/`Divider`|
// |                                  | /`Section("Label")`, shortcuts via `.keyboardShortcut`        |
// | Tooltip (500 ms, kbd hint)       | `.kzHelp(_:shortcut:)` → `.help`                              |
// | Popover / Dialog / AlertDialog / | `.kzPopover`, `.sheet` + `DialogLayout`, `.alert`,            |
// | Sheet                            | `.confirmationDialog`                                         |
// | ScrollArea                       | `ScrollView` (native overlay scrollers)                      |
// | Separator                        | `Hairline` (`Divider` only inside menus)                      |

import AppKit
import KudzuCore
import SwiftUI

/// The regular and compact native control sizes.
public enum KZControlSize: Sendable { case sm, `default` }

extension View {
    /// `Switch`: the native switch, iris when on; `size: .sm` is the 24×14 variant (mini).
    public func kzSwitch(size: KZControlSize = .default) -> some View {
        modifier(TintedToggle(style: .switch, size: size))
    }

    /// `Checkbox`: the native checkbox in the primary colour.
    public func kzCheckbox() -> some View {
        modifier(TintedToggle(style: .checkbox, size: .default))
    }

    /// `RadioGroup`: a radio-group picker, primary tint, 8 pt between options (`gap-2`).
    public func kzRadioGroup() -> some View {
        modifier(TintedPicker(kind: .radio))
    }

    /// `Select`: a pop-up button (`.menu` picker) sized like `SelectTrigger` (`h-8`, sm `h-7`).
    /// `width` gives the trigger a fixed width (`SelectTrigger className="w-32"`); otherwise it hugs
    /// its title.
    public func kzSelect(size: KZControlSize = .default, width: CGFloat? = nil) -> some View {
        modifier(TintedPicker(kind: .menu(size), width: width))
    }

    /// A `Menu` drawn as a Kudzu button (`DropdownMenuTrigger asChild <Button …>`). Menu triggers
    /// don't press-scale (`[aria-haspopup]` is exempt), and the system draws the menu.
    public func kzMenuTrigger(_ variant: ButtonVariant = .ghost, size: ButtonSize = .default, indicator: Bool = false, fullWidth: Bool = false) -> some View {
        self
            .menuStyle(.button)
            .buttonStyle(KZButtonStyle.menuTrigger(variant, size: size))
            .menuIndicator(indicator ? .visible : .hidden)
            .fixedSize(horizontal: !fullWidth, vertical: true)
            .frame(maxWidth: fullWidth ? .infinity : nil)
    }

    /// `Tooltip`: a native help tag, with the shortcut written after the text the way the web's
    /// keycap reads (`"New snippet ⌘N"`).
    public func kzHelp(_ text: String, shortcut: String? = nil) -> some View {
        help(shortcut.map { "\(text) \(Keys.keyLabels($0).joined())" } ?? text)
    }

    /// `Popover`: a native popover whose content gets the web's padding and gap (`p-2.5 gap-2.5`,
    /// `w-72` unless the content sizes itself).
    public func kzPopover<Content: View>(isPresented: Binding<Bool>, arrowEdge: Edge = .bottom, width: CGFloat? = 288, @ViewBuilder content: @escaping () -> Content) -> AnyView {
        // Keep the SDK's client-emitted popover opaque type inside this module. Exposing it
        // through our own opaque result leaves an unresolved descriptor in optimized clients.
        AnyView(popover(isPresented: isPresented, arrowEdge: arrowEdge) {
            VStack(alignment: .leading, spacing: 10) { content() }
                .font(.system(size: 14))
                .padding(10)
                .frame(width: width, alignment: .leading)
        })
    }
}

struct TintedToggle: ViewModifier {
    enum Style { case `switch`, checkbox }
    var style: Style
    var size: KZControlSize
    @Environment(\.colorScheme) private var scheme

    func body(content: Content) -> some View {
        let t = KZTheme(scheme)
        switch style {
        case .switch:
            content.toggleStyle(.switch).tint(t[.iris]).controlSize(size == .sm ? .mini : .small)
        case .checkbox:
            content.toggleStyle(.checkbox).tint(t[.primary])
        }
    }
}

struct TintedPicker: ViewModifier {
    enum Kind { case radio, menu(KZControlSize) }
    var kind: Kind
    var width: CGFloat? = nil
    @Environment(\.colorScheme) private var scheme

    func body(content: Content) -> some View {
        let t = KZTheme(scheme)
        switch kind {
        case .radio:
            content.pickerStyle(.radioGroup).tint(t[.primary])
        case .menu(let size):
            if let width {
                content.pickerStyle(.menu).labelsHidden().controlSize(size == .sm ? .small : .regular).frame(width: width)
            } else {
                content.pickerStyle(.menu).labelsHidden().fixedSize().controlSize(size == .sm ? .small : .regular)
            }
        }
    }
}

// MARK: Dialog layout

/// The inside of a `Dialog` / `AlertDialog` shown with `.sheet`: title (16 pt medium), description
/// (14 pt muted), body, and the footer bar (`border-t bg-muted/50 p-4`, actions trailing).
public struct DialogLayout<Content: View, Footer: View>: View {
    var title: String
    var description: String?
    var content: Content
    var footer: Footer
    /// `sm:max-w-sm` (384) for dialogs, 320 for small alert dialogs.
    var width: CGFloat
    /// The gap between header, body and footer (`DialogContent gap-4`; some dialogs set `gap-5`).
    var spacing: CGFloat
    /// The gap between title and description (`DialogHeader gap-2`).
    var headerSpacing: CGFloat
    @Environment(\.colorScheme) private var scheme

    /// Creates DialogLayout with the supplied content and styling.
    public init(_ title: String, description: String? = nil, width: CGFloat = 384, spacing: CGFloat = 16, headerSpacing: CGFloat = 8, @ViewBuilder content: () -> Content, @ViewBuilder footer: () -> Footer) {
        self.title = title
        self.description = description
        self.width = width
        self.spacing = spacing
        self.headerSpacing = headerSpacing
        self.content = content()
        self.footer = footer()
    }

    /// The composed native view.
    public var body: some View {
        let t = KZTheme(scheme)
        VStack(alignment: .leading, spacing: spacing) {
            VStack(alignment: .leading, spacing: headerSpacing) {
                Text(title).font(.system(size: 16, weight: .medium))
                if let description {
                    Text(description).font(.system(size: 14)).mutedForeground().fixedSize(horizontal: false, vertical: true)
                }
            }
            content
            HStack(spacing: 8) {
                Spacer(minLength: 0)
                footer
            }
            .padding(16)
            .frame(maxWidth: .infinity)
            .background(t[.muted, alpha: 0.5])
            .overlay(alignment: .top) { Hairline(token: .border) }
            .padding(.horizontal, -16)
            .padding(.bottom, -16)
        }
        .padding(16)
        .frame(width: width)
        .background(t[.popover])
    }
}

// MARK: Lists and forms

extension View {
    /// Row styling for `List` rows built from the `Item` family: no inset separators, the web's
    /// 8 pt horizontal inset, and rounded selection supplied by the list style.
    public func kzListRow(insets: EdgeInsets = EdgeInsets(top: 0, leading: 8, bottom: 0, trailing: 8)) -> some View {
        self
            .listRowSeparator(.hidden)
            .listRowInsets(insets)
    }

    /// For rows that draw their own selection (the sidebar pill, the snippet list's iris row): turns
    /// off the enclosing table's accent-colored highlight. The `List` keeps its selection, keyboard
    /// navigation and drag; only AppKit's drawing of the selected row goes.
    public func kzOwnSelection() -> some View {
        background(TableHighlightOff())
    }

    /// Settings pages: a grouped `Form` with Kudzu's 13 pt body and scroll background removed so the
    /// card surface shows.
    public func kzForm() -> some View {
        self
            .formStyle(.grouped)
            .scrollContentBackground(.hidden)
            .font(.system(size: 13))
    }
}

/// How a `SettingsRow` sits in its container.
public enum SettingsRowLayout: Sendable {
    /// A row of a settings card (`Row`: `min-h-14 items-center gap-6 py-3`).
    case card
    /// A label/control pair inside a dialog (`flex items-center justify-between gap-4`).
    case inline
}

/// A settings row: title and optional description on the leading side, the control trailing
/// (`shrink-0 items-center gap-2`).
public struct SettingsRow<Control: View>: View {
    var title: String
    var description: AnyView?
    var layout: SettingsRowLayout
    var control: Control

    /// Creates SettingsRow with the supplied content and styling.
    public init(_ title: String, description: String? = nil, layout: SettingsRowLayout = .card, @ViewBuilder control: () -> Control) {
        self.title = title
        self.description = description.map { AnyView(Text($0)) }
        self.layout = layout
        self.control = control()
    }

    /// A row whose description is a view, e.g. a `ShortcutText` with keycaps or a status line.
    public init<Description: View>(_ title: String, layout: SettingsRowLayout = .card, @ViewBuilder description: () -> Description, @ViewBuilder control: () -> Control) {
        self.title = title
        self.description = AnyView(description())
        self.layout = layout
        self.control = control()
    }

    /// The composed native view.
    public var body: some View {
        HStack(spacing: layout == .card ? 24 : 16) {
            VStack(alignment: .leading, spacing: 0) {
                Text(title)
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(.kz.foreground)
                if let description {
                    description
                        .font(.system(size: 12))
                        .foregroundStyle(.kz.mutedForeground)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 8) { control }
                .fixedSize()
        }
        .padding(.vertical, layout == .card ? 12 : 0)
        .frame(minHeight: layout == .card ? 56 : nil)
    }
}

/// Finds the `NSTableView` behind a `List` row and sets `selectionHighlightStyle` to `.none`.
private struct TableHighlightOff: NSViewRepresentable {
    func makeNSView(context: Context) -> Probe { Probe() }
    func updateNSView(_ view: Probe, context: Context) {}

    final class Probe: NSView {
        override func hitTest(_ point: NSPoint) -> NSView? { nil }

        override func viewDidMoveToSuperview() {
            super.viewDidMoveToSuperview()
            apply()
        }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            apply()
        }

        private func apply() {
            var view = superview
            while let current = view, !(current is NSTableView) { view = current.superview }
            guard let table = view as? NSTableView, table.selectionHighlightStyle != .none else { return }
            table.selectionHighlightStyle = .none
        }
    }
}
