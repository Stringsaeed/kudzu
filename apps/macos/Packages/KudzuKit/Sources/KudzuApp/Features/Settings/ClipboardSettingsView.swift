import KudzuCore
import KudzuDesign
import SwiftUI

/// Clipboard settings section (`settings.tsx` `ClipboardSettings`).
struct ClipboardSettingsView: View {
    @Environment(AppModel.self) private var model
    @State private var customMode: Bool = false
    @State private var customDraft: String = "30"
    @State private var confirmClearOpen: Bool = false
    @FocusState private var customFocused: Bool

    private var retentionDays: UInt32 { model.settings.clipboardRetentionDays }
    private var retentionChoice: RetentionChoice {
        RetentionChoice.selection(days: retentionDays, customMode: customMode)
    }
    private var pinnedCount: Int {
        model.clipboard.entries.filter(\.pinned).count
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 32) {
            SettingsCardGroup(
                "Clipboard history",
                description: "Kudzu remembers what you copy, so you can find it and paste it again. History stays on this Mac. It is never synced or uploaded."
            ) {
                SettingsRow(
                    "Record what I copy",
                    description: "Text, links, images, files and colors, with the time each was copied."
                ) {
                    Toggle(
                        "Clipboard history",
                        isOn: Binding(
                            get: { model.settings.clipboardEnabled },
                            set: { next in Task { await model.patchSettings { $0.clipboardEnabled = next } } }
                        )
                    )
                    .kzSwitch()
                    .labelsHidden()
                }

                Hairline()

                SettingsRow("Shortcut") {
                    ShortcutText([.text("Opens your history over any app."), .shortcut("enter"), .text("pastes,"), .shortcut("mod+enter"), .text("only copies.")])
                } control: {
                    HotkeyRecorder(value: model.settings.clipboardHotkey, label: "Clipboard history shortcut") { next in
                        await model.applyClipboardHotkey(next)
                    }
                }

                Hairline()

                SettingsRow(
                    "Keep history",
                    description: RetentionChoice.description(days: retentionDays)
                ) {
                    HStack(spacing: 8) {
                        if retentionChoice == .custom {
                            // `flex items-center gap-1.5 text-xs`; the field is `h-8 w-20 text-center tabular`.
                            HStack(spacing: 6) {
                                TextField("Days", text: $customDraft)
                                    .multilineTextAlignment(.center)
                                    .font(.system(size: 14).monospacedDigit())
                                    .kzInput()
                                    .frame(width: 80)
                                    .focused($customFocused)
                                    .onSubmit { commitCustomDays() }
                                    .onChange(of: customFocused) { _, focused in
                                        if !focused { commitCustomDays() }
                                    }
                                    .accessibilityLabel("Days to keep history")
                                Text("days")
                                    .font(.system(size: 12))
                                    .mutedForeground()
                            }
                        }
                        Picker(
                            "Keep history",
                            selection: Binding(
                                get: { retentionChoice },
                                set: { next in
                                    switch next {
                                    case .custom:
                                        customMode = true
                                        customDraft = RetentionChoice.draft(for: retentionDays)
                                    case let .preset(days):
                                        customMode = false
                                        Task { await model.patchSettings { $0.clipboardRetentionDays = days } }
                                    }
                                }
                            )
                        ) {
                            ForEach(RetentionChoice.presets, id: \.days) { option in
                                Text(option.label).tag(RetentionChoice.preset(option.days))
                            }
                            Text("Custom…").tag(RetentionChoice.custom)
                        }
                        .kzSelect(width: 128)
                    }
                }

                Hairline()

                SettingsRow("Paste as plain text") {
                    ShortcutText([.text("Drops fonts, colors and links from formatted text. Hold"), .shortcut("shift"), .text("to do the opposite.")])
                } control: {
                    Toggle(
                        "Paste as plain text",
                        isOn: Binding(
                            get: { model.settings.clipboardPastePlain },
                            set: { next in Task { await model.patchSettings { $0.clipboardPastePlain = next } } }
                        )
                    )
                    .kzSwitch()
                    .labelsHidden()
                }
            }
            .stagger(0)

            SettingsCardGroup(
                "Privacy",
                description: "Password managers are excluded. Kudzu also skips anything an app marks as private or temporary, like passwords and one-time codes."
            ) {
                SettingsRow(
                    "Show secrets",
                    description: "Text that looks like a password, API key or token is hidden until you hover over it. Choose “Not a secret?” on an item to correct Kudzu."
                ) {
                    Toggle(
                        "Show secrets",
                        isOn: Binding(
                            get: { model.settings.clipboardShowSecrets },
                            set: { next in Task { await model.patchSettings { $0.clipboardShowSecrets = next } } }
                        )
                    )
                    .kzSwitch()
                    .labelsHidden()
                }

                Hairline()

                AppPicker(
                    value: Binding(
                        get: { model.settings.clipboardIgnoredApps },
                        set: { next in Task { await model.patchSettings { $0.clipboardIgnoredApps = next } } }
                    ),
                    addLabel: "Never record from an app"
                )
                .padding(.vertical, 12)
            }
            .stagger(1)

            SettingsCardGroup("Storage") {
                SettingsRow(
                    ClipboardStorageCopy.title(
                        count: model.clipboard.entries.count,
                        diskUsage: model.clipboard.diskUsage
                    ),
                    description: ClipboardStorageCopy.description(pinned: pinnedCount)
                ) {
                    Button("Clear history…") {
                        confirmClearOpen = true
                    }
                    .buttonStyle(.kz(.secondary, size: .sm))
                    .disabled(model.clipboard.entries.isEmpty)
                }
            }
            .stagger(2)
        }
        .onAppear {
            customMode = !RetentionChoice.isPreset(retentionDays)
            customDraft = RetentionChoice.draft(for: retentionDays)
        }
        .sheet(isPresented: $confirmClearOpen) {
            clearHistorySheet
        }
    }

    private var clearHistorySheet: some View {
        DialogLayout(
            "Clear clipboard history?",
            description: ClipboardStorageCopy.clearMessage(pinned: pinnedCount)
        ) {
            EmptyView()
        } footer: {
            Button("Cancel") {
                confirmClearOpen = false
            }
            .buttonStyle(.kz(.outline, size: .sm))
            .keyboardShortcut(.cancelAction)

            Button("Clear history") {
                confirmClearOpen = false
                Task {
                    do {
                        let removed = try await model.clearClipboard(includePinned: false)
                        toast.success(ClipboardStorageCopy.cleared(removed))
                    } catch {
                        toast.error(AppModel.errorMessage(error))
                    }
                }
            }
            .buttonStyle(.kz(.destructive, size: .sm))
            .keyboardShortcut(.defaultAction)
        }
        .frame(width: 400)
    }

    private func commitCustomDays() {
        if let parsed = RetentionChoice.parseCustom(customDraft) {
            customDraft = String(parsed)
            Task { await model.patchSettings { $0.clipboardRetentionDays = parsed } }
        } else {
            customDraft = RetentionChoice.draft(for: retentionDays)
        }
    }
}
