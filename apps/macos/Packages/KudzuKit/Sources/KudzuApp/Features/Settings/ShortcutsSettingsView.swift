import KudzuCore
import KudzuDesign
import SwiftUI

/// The `.shortcuts` section inside `SettingsScreen` (`shortcuts.tsx` `ShortcutsSection`).
struct ShortcutsSettingsView: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            GlobalAcceleratorLine().stagger(0)
            ShortcutsContentView().stagger(1)
        }
    }
}

/// "From any app: search ⌘K · clipboard ⌘⇧V" (`shortcuts.tsx` dialog header).
struct GlobalAcceleratorLine: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        HStack(spacing: 8) {
            Text("From any app: search")
                .font(.system(size: 12))
                .mutedForeground()
            Shortcut(keys: Keys.acceleratorLabels(model.settings.paletteHotkey), tone: .accent)
            if model.settings.clipboardEnabled {
                Text("·")
                    .font(.system(size: 12))
                    .foregroundStyle(.kz.faint)
                Text("clipboard")
                    .font(.system(size: 12))
                    .mutedForeground()
                Shortcut(keys: Keys.acceleratorLabels(model.settings.clipboardHotkey), tone: .accent)
            }
            Spacer(minLength: 0)
        }
    }
}

/// Two-column grid of shortcut categories and keycap bindings (`shortcuts.tsx` `ShortcutsContent`).
public struct ShortcutsContentView: View {
    @Environment(AppModel.self) private var model

    public init() {}

    public var body: some View {
        let sections = ShortcutsCatalog.sections(settings: model.settings)
        LazyVGrid(
            columns: [
                GridItem(.flexible(minimum: 220), spacing: 14, alignment: .top),
                GridItem(.flexible(minimum: 220), spacing: 14, alignment: .top),
            ],
            alignment: .leading,
            spacing: 14
        ) {
            ForEach(sections) { section in
                VStack(alignment: .leading, spacing: 6) {
                    Text(section.title)
                        .font(.system(size: 11, weight: .medium))
                        .mutedForeground()
                        .padding(.horizontal, 4)

                    VStack(spacing: 0) {
                        ForEach(Array(section.rows.enumerated()), id: \.element.id) { index, row in
                            if index > 0 {
                                Hairline()
                            }
                            HStack(spacing: 12) {
                                Text(row.label)
                                    .font(.system(size: 12))
                                    .foregroundStyle(Color.kz.mix(.foreground, alpha: 0.85))
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                ShortcutBindingView(binding: row.binding)
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 8)
                        }
                    }
                    .depth(.card, fill: Color.kz.surface, in: RoundedRectangle(cornerRadius: Radius.lg, style: .continuous))
                }
            }
        }
    }
}

/// Modal sheet presenting the keyboard shortcuts reference (`shortcuts.tsx` `ShortcutsDialog`).
public struct ShortcutsDialogView: View {
    @Environment(AppModel.self) private var model

    public init() {}

    public var body: some View {
        DialogLayout(
            "Keyboard shortcuts",
            description: "Every shortcut in Kudzu.",
            width: 720
        ) {
            VStack(alignment: .leading, spacing: 16) {
                GlobalAcceleratorLine()
                ScrollView {
                    ShortcutsContentView()
                }
                .frame(maxHeight: 480)
            }
        } footer: {
            Button("Done") {
                model.shortcutsOpen = false
            }
            .buttonStyle(.kz(.default, size: .sm))
            .keyboardShortcut(.cancelAction)
        }
        .frame(width: 660)
    }
}

// MARK: - Binding renderer

private struct ShortcutBindingView: View {
    let binding: ShortcutsCatalog.Binding

    var body: some View {
        switch binding {
        case let .key(chord):
            Shortcut(chord)

        case let .alternatives(chords):
            HStack(spacing: 4) {
                ForEach(Array(chords.enumerated()), id: \.offset) { index, chord in
                    if index > 0 {
                        Text("/")
                            .font(.system(size: 10))
                            .mutedForeground()
                    }
                    Shortcut(chord)
                }
            }

        case let .range(first, last):
            HStack(spacing: 4) {
                Shortcut(first)
                Text("…")
                    .font(.system(size: 11))
                    .mutedForeground()
                Shortcut(last)
            }

        case let .modifierClicks(mods):
            HStack(spacing: 4) {
                ForEach(Array(mods.enumerated()), id: \.offset) { index, mod in
                    if index > 0 {
                        Text(",")
                            .font(.system(size: 11))
                            .mutedForeground()
                    }
                    Shortcut(mod)
                    Text("+ click")
                        .font(.system(size: 11))
                        .mutedForeground()
                }
            }

        case let .globalAccelerator(raw):
            HStack(spacing: 3) {
                ForEach(Array(Keys.acceleratorLabels(raw).enumerated()), id: \.offset) { _, key in
                    Kbd(key)
                }
            }
        }
    }
}
