import KudzuCore
import KudzuDesign
import SwiftUI

/// General settings section (`settings.tsx` `General` + `ICloudSync`).
struct GeneralSettingsView: View {
    @Environment(AppModel.self) private var model
    @State private var syncStatus: SyncStatus?

    private var showsMacControls: Bool {
        model.platform == .macos || model.platform == .web
    }

    var body: some View {
        let lead = showsMacControls ? 1 : 0
        VStack(alignment: .leading, spacing: 32) {
            if showsMacControls {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Permissions")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.kz.foreground)
                        .padding(.horizontal, 4)
                        .accessibilityAddTraits(.isHeader)
                    PermissionSetupView()
                }
                .stagger(0)
            }

            SettingsCardGroup("Quick search") {
                SettingsRow("Shortcut") {
                    ShortcutText([.text("Opens the search tray over any app."), .shortcut("enter"), .text("inserts,"), .shortcut("mod+enter"), .text("copies.")])
                } control: {
                    HotkeyRecorder(value: model.settings.paletteHotkey) { next in
                        await model.applyPaletteHotkey(next)
                    }
                }
            }
            .stagger(lead)

            iCloudSyncGroup
                .stagger(lead + 1)

            SettingsCardGroup("App") {
                SettingsRow("Appearance") {
                    Segmented(
                        "Appearance",
                        selection: Binding(
                            get: { model.settings.theme },
                            set: { next in Task { await model.selectTheme(next) } }
                        ),
                        options: [
                            SegmentOption("Dark", value: Theme.dark),
                            SegmentOption("Light", value: Theme.light),
                            SegmentOption("System", value: Theme.system),
                        ]
                    )
                }

                Hairline()

                SettingsRow(
                    "Open at login",
                    description: "Kudzu lives in the menu bar and starts with your Mac."
                ) {
                    Toggle(
                        "Open at login",
                        isOn: Binding(
                            get: { model.settings.launchAtLogin },
                            set: { next in Task { await model.patchSettings { $0.launchAtLogin = next } } }
                        )
                    )
                    .kzSwitch()
                    .labelsHidden()
                }

                if model.platform != .windows && model.platform != .linux {
                    Hairline()

                    SettingsRow(
                        "Show in Dock while open",
                        description: "Otherwise Kudzu only appears in the menu bar."
                    ) {
                        Toggle(
                            "Show in Dock while open",
                            isOn: Binding(
                                get: { model.settings.showInDock },
                                set: { next in Task { await model.patchSettings { $0.showInDock = next } } }
                            )
                        )
                        .kzSwitch()
                        .labelsHidden()
                    }
                }
            }
            .stagger(lead + 2)
        }
        .task {
            syncStatus = await model.loadSyncStatus()
        }
        .onChange(of: model.syncStatus) { _, updated in
            syncStatus = updated
        }
    }

    /// `ICloudSync`: one row. The status line is the row's description under "Sync with iCloud";
    /// the trailing side is the optional action ("Update" or "Check iCloud status") and then the switch.
    private var iCloudSyncGroup: some View {
        let effectiveStatus = syncStatus ?? model.syncStatus
        let line = SyncLine.of(effectiveStatus)

        return SettingsCardGroup(
            "iCloud",
            description: "The same snippets on every Mac signed in to your iCloud. Stored in your private iCloud database; Kudzu has no servers of its own."
        ) {
            SettingsRow("Sync with iCloud") {
                HStack(spacing: 6) {
                    if line.busy {
                        ProgressView().controlSize(.mini)
                    } else if line.check {
                        Icon.check
                            .icon(12)
                            .foregroundStyle(.kz.success)
                            .checkIn()
                    }
                    Text(line.text)
                        .foregroundStyle(toneColor(line.tone))
                        .fixedSize(horizontal: false, vertical: true)
                }
                .accessibilityElement(children: .combine)
            } control: {
                switch line.action {
                case .update:
                    Button("Update") {
                        model.setSettingsSection(.about)
                    }
                    .buttonStyle(.kz(.secondary, size: .sm))
                case .syncNow:
                    syncNowButton(disabled: false)
                case nil:
                    EmptyView()
                }

                Toggle(
                    "Sync with iCloud",
                    isOn: Binding(
                        get: { model.settings.icloudSync },
                        set: { next in
                            Task {
                                await model.patchSettings { $0.icloudSync = next }
                                syncStatus = await model.loadSyncStatus()
                            }
                        }
                    )
                )
                .kzSwitch()
                .labelsHidden()
            }
            .onChange(of: line.text) { _, text in
                // `aria-live="polite"` on the status line.
                AccessibilityNotification.Announcement(text).post()
            }
        }
    }

    private func syncNowButton(disabled: Bool) -> some View {
        Button {
            Task {
                await model.triggerSyncNow()
                syncStatus = await model.loadSyncStatus()
            }
        } label: {
            ButtonLabel("Check iCloud status", icon: Icon.refreshCw)
        }
        .buttonStyle(.kz(.ghost, size: .sm))
        .disabled(disabled)
        .accessibilityLabel("Check iCloud status")
    }

    private func toneColor(_ tone: SyncLine.Tone) -> Color {
        switch tone {
        // The line is the row description (`text-xs text-muted-foreground`); an empty tone inherits it.
        case .plain, .muted: .kz.mutedForeground
        case .warning: .kz.warning
        case .destructive: .kz.destructive
        }
    }
}

/// Shared card container with a small section header used across Settings views.
/// `settings.tsx` `Group`: the title and optional description sit above the card, not inside it.
struct SettingsCardGroup<Content: View, HeaderExtra: View>: View {
    var title: String
    var description: String?
    var content: Content
    var headerExtra: HeaderExtra

    init(
        _ title: String,
        description: String? = nil,
        @ViewBuilder headerExtra: () -> HeaderExtra = { EmptyView() },
        @ViewBuilder content: () -> Content
    ) {
        self.title = title
        self.description = description
        self.headerExtra = headerExtra()
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 0) {
                    Text(title)
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.kz.foreground)
                    if let description {
                        Text(description)
                            .font(.system(size: 12))
                            .mutedForeground()
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                Spacer(minLength: 0)
                headerExtra
            }
            .padding(.horizontal, 4)

            // `divide-y divide-hairline rounded-xl bg-surface px-4 shadow-card`: rows carry their own
            // vertical padding, and `Hairline()`s between them stand in for `divide-y`.
            VStack(alignment: .leading, spacing: 0) {
                content
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 16)
            .depth(.card, fill: Color.kz.surface, in: RoundedRectangle(cornerRadius: Radius.xl, style: .continuous))
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(title)
    }
}
