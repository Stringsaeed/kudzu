import KudzuCore
import KudzuDesign
import SwiftUI

/// Expansion settings section (`settings.tsx` `Expansion`).
struct ExpansionSettingsView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        VStack(alignment: .leading, spacing: 32) {
            SettingsCardGroup("Expanding") {
                SettingsRow("Pause all snippets") {
                    ShortcutText([.text("Handy while gaming or presenting."), .shortcut("mod+shift+p"), .text("also works.")])
                } control: {
                    Toggle(
                        "Pause all snippets",
                        isOn: Binding(
                            get: { model.status.paused },
                            set: { next in Task { try? await model.setPaused(next) } }
                        )
                    )
                    .kzSwitch()
                    .labelsHidden()
                }

                Hairline()

                SettingsRow(
                    "Trigger prefix",
                    description: "Suggested at the start of new triggers so they don't fire mid-sentence."
                ) {
                    TextField(
                        "Prefix",
                        text: Binding(
                            get: { model.settings.defaultTriggerPrefix },
                            set: { raw in
                                let next = String(raw.prefix(4))
                                guard next != model.settings.defaultTriggerPrefix else { return }
                                Task { await model.patchSettings { $0.defaultTriggerPrefix = next } }
                            }
                        )
                    )
                    .multilineTextAlignment(.center)
                    .font(KZFonts.mono(size: 14))
                    .kzInput()
                    .frame(width: 80)
                    .accessibilityLabel("Trigger prefix")
                }

                Hairline()

                SettingsRow(
                    "Insert by",
                    description: "Default for every snippet. Typing is most compatible; pasting is fastest for long text."
                ) {
                    Segmented(
                        "Default insertion method",
                        selection: Binding(
                            get: { model.settings.defaultInsertion },
                            set: { next in Task { await model.patchSettings { $0.defaultInsertion = next } } }
                        ),
                        options: [
                            SegmentOption("Automatic", value: InsertionMethod.automatic),
                            SegmentOption("Type", value: InsertionMethod.type),
                            SegmentOption("Paste", value: InsertionMethod.paste),
                        ]
                    )
                }
            }
            .stagger(0)

            // `flex flex-col gap-2 py-3 text-xs leading-relaxed`.
            SettingsCardGroup("Good to know") {
                VStack(alignment: .leading, spacing: 8) {
                    note("Undo an expansion", "press Backspace right after it to get your trigger back.")
                    note("Clipboard is safe", "when Kudzu pastes, it restores whatever you had copied, and marks its own text so clipboard managers ignore it.")
                    note("Secure Input", "while a password field is focused, macOS blocks all text expanders. Kudzu shows this in the menu bar.")
                }
                .font(.system(size: 12))
                .lineSpacing(5)
                .padding(.vertical, 12)
            }
            .stagger(1)
        }
    }

    private func note(_ title: String, _ body: String) -> some View {
        Text("\(Text(title).fontWeight(.medium).foregroundStyle(Color.kz.foreground)) — \(body)")
            .foregroundStyle(.kz.mutedForeground)
            .fixedSize(horizontal: false, vertical: true)
    }
}
