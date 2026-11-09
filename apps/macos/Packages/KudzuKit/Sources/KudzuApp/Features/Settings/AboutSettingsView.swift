import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

/// About settings section (`settings.tsx` `About` + `UpdateRow`).
struct AboutSettingsView: View {
    @Environment(AppModel.self) private var model
    @State private var version: String?
    @State private var updateState: UpdateState?

    private var effectiveUpdateState: UpdateState {
        updateState ?? model.updateState
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 32) {
            brand

            SettingsCardGroup("Updates") {
                UpdateRowView(state: effectiveUpdateState) { next in
                    updateState = next
                }
                Hairline()
                SettingsRow(
                    "Check for updates automatically",
                    description: "Looks a few times a day and downloads quietly. Kudzu never restarts without you."
                ) {
                    Toggle(
                        "Check for updates automatically",
                        isOn: Binding(
                            get: { model.settings.autoCheckUpdates },
                            set: { next in Task { await model.patchSettings { $0.autoCheckUpdates = next } } }
                        )
                    )
                    .kzSwitch()
                    .labelsHidden()
                }
                Hairline()
                SettingsRow(
                    "Get beta updates",
                    description: "Try new versions early. You’ll also get every stable release."
                ) {
                    Toggle(
                        "Get beta updates",
                        isOn: Binding(
                            get: { model.settings.updateChannel == .beta },
                            set: { on in
                                Task { await model.patchSettings { $0.updateChannel = on ? .beta : .stable } }
                            }
                        )
                    )
                    .kzSwitch()
                    .labelsHidden()
                }
            }

            HStack(alignment: .top, spacing: 8) {
                Icon.shieldCheck
                    .icon(16)
                    .foregroundStyle(.kz.success)
                    .padding(.top, 1)
                Text(BrandCopy.privacyBody)
                    .font(.system(size: 12))
                    .lineSpacing(5)
                    .mutedForeground()
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .depth(.card, fill: Color.kz.surface, in: RoundedRectangle(cornerRadius: Radius.xl, style: .continuous))
        }
        // The page is one block on the web (`<div className="flex flex-col gap-8">`), so it rises as one.
        .stagger(0)
        .task {
            let loaded = await model.loadAboutInfo()
            version = loaded.version
            updateState = loaded.updateState
        }
        .onChange(of: model.updateState) { _, next in
            updateState = next
        }
    }

    private var brand: some View {
        VStack(spacing: 16) {
            if let icon = NSImage(named: NSImage.applicationIconName) {
                Image(nsImage: icon)
                    .resizable()
                    .interpolation(.high)
                    .frame(width: 88, height: 88)
                    .shadow(color: Color.oklch(0.488, 0.243, 264.376, alpha: 0.3), radius: 12, y: 12)
                    .padding(.vertical, -8)
            }
            VStack(spacing: 8) {
                Wordmark(.small)
                    .frame(width: 144)
                Text("Version \(version ?? "…") · MIT licensed")
                    .font(.system(size: 12))
                    .mutedForeground()
            }
            VStack(spacing: 0) {
                Text(BrandCopy.tagline)
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(.kz.foreground)
                Text(BrandCopy.description)
                    .font(.system(size: 14))
                    .mutedForeground()
            }
            .multilineTextAlignment(.center)
            .lineSpacing(6)
            .frame(maxWidth: 384)
        }
        .frame(maxWidth: .infinity)
        .padding(.top, 24)
    }
}

// MARK: - Update row

private struct UpdateRowView: View {
    @Environment(AppModel.self) private var model
    var state: UpdateState
    var setState: (UpdateState) -> Void

    var body: some View {
        switch UpdateRowContent.of(state) {
        case let .ready(version, sections, restarting):
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .center, spacing: 24) {
                    VStack(alignment: .leading, spacing: 0) {
                        HStack(spacing: 8) {
                            Icon.sparkles.icon(14).foregroundStyle(.kz.iris)
                            Text("Kudzu \(version) is ready")
                                .font(.system(size: 13, weight: .medium))
                                .foregroundStyle(.kz.foreground)
                        }
                        Text(restarting
                            ? "Installing… Kudzu will reopen by itself."
                            : "Downloaded and verified. It installs when you quit Kudzu, or restart now. Takes a second.")
                            .font(.system(size: 12))
                            .mutedForeground()
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    if restarting {
                        ProgressView().controlSize(.small)
                    } else {
                        Button {
                            Task {
                                setState(.restarting(version: version))
                                if let err = await model.triggerRestartToUpdate() {
                                    setState(err)
                                    if case let .error(message, _) = err {
                                        toast.error("Couldn’t install the update", description: message)
                                    }
                                }
                            }
                        } label: {
                            ButtonLabel("Restart to update", icon: Icon.refreshCw)
                        }
                        .buttonStyle(.kz(.default, size: .sm))
                    }
                }

                if !sections.isEmpty {
                    ReleaseNotesWell(sections: sections)
                }

                ReleaseNotesLink()
            }
            .padding(.vertical, 14)

        case let .downloading(version, percent):
            VStack(alignment: .leading, spacing: 10) {
                VStack(alignment: .leading, spacing: 0) {
                    Text("Downloading Kudzu \(version)")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(.kz.foreground)
                    Text(UpdateRowContent.downloadingDetail(percent: percent))
                        .font(.system(size: 12))
                        .mutedForeground()
                }
                // `h-1 rounded-full bg-foreground/[0.06] shadow-well`, the bar `transition-[width] duration-300`.
                GeometryReader { geo in
                    let fraction = CGFloat(percent ?? 30) / 100
                    Color.clear
                        .depth(.well, fill: Color.kz.mix(.foreground, alpha: 0.06), in: Capsule())
                        .overlay(alignment: .leading) {
                            Capsule()
                                .fill(Color.kz.iris)
                                .frame(width: max(4, geo.size.width * fraction))
                                .animation(.timingCurve(0.4, 0, 0.2, 1, duration: 0.3), value: fraction)
                        }
                        .clipShape(Capsule())
                }
                .frame(height: 4)
                .accessibilityElement()
                .accessibilityLabel("Download progress")
                .accessibilityValue(percent.map { "\($0) percent" } ?? "Downloading")
            }
            .padding(.vertical, 14)

        case let .row(description, tone, button, checking):
            HStack(alignment: .center, spacing: 24) {
                VStack(alignment: .leading, spacing: 0) {
                    Text("Software update")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(.kz.foreground)
                    Text(description)
                        .font(.system(size: 12))
                        .foregroundStyle(toneColor(tone))
                        .fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                Button {
                    Task {
                        setState(.checking)
                        setState(await model.triggerUpdateCheck())
                    }
                } label: {
                    if checking {
                        HStack(spacing: 6) {
                            ProgressView().controlSize(.mini)
                            Text(button)
                        }
                    } else {
                        ButtonLabel(button, icon: Icon.refreshCw)
                    }
                }
                .buttonStyle(.kz(.secondary, size: .sm))
                .disabled(checking)
            }
            .padding(.vertical, 12)
            .frame(minHeight: 56)
        }
    }

    private func toneColor(_ tone: SyncLine.Tone) -> Color {
        switch tone {
        case .plain, .muted: .kz.mutedForeground
        case .warning: .kz.warning
        case .destructive: .kz.destructive
        }
    }
}

/// The ready update's notes (`selectable max-h-48 overflow-y-auto rounded-lg bg-foreground/[0.025]
/// px-3 py-2.5 text-xs leading-relaxed shadow-well`): grows with the notes up to 192 pt, then scrolls.
private struct ReleaseNotesWell: View {
    var sections: [ReleaseNotes.Section]
    @State private var contentHeight: CGFloat = 0

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: Radius.lg, style: .continuous)
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(Array(sections.enumerated()), id: \.offset) { _, section in
                    VStack(alignment: .leading, spacing: 2) {
                        if let heading = section.heading {
                            Text(heading)
                                .font(.system(size: 12, weight: .medium))
                                .foregroundStyle(Color.kz.mix(.foreground, alpha: 0.9))
                        }
                        ForEach(Array(section.items.enumerated()), id: \.offset) { _, item in
                            HStack(alignment: .firstTextBaseline, spacing: 6) {
                                Text("•").foregroundStyle(.kz.faint)
                                Text(item)
                                    .foregroundStyle(.kz.mutedForeground)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                            .font(.system(size: 12))
                            .lineSpacing(5)
                        }
                    }
                }
            }
            .textSelection(.enabled)
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { contentHeight = $0 }
        }
        .frame(height: min(max(contentHeight, 1), 192))
        .depth(.well, fill: Color.kz.mix(.foreground, alpha: 0.025), in: shape)
        .clipShape(shape)
    }
}

/// "All release notes" (`text-xs text-muted-foreground underline-offset-2 hover:text-foreground hover:underline`).
private struct ReleaseNotesLink: View {
    @State private var hovered = false

    var body: some View {
        Button {
            NSWorkspace.shared.open(ReleaseNotes.releasesURL)
        } label: {
            Text("All release notes")
                .font(.system(size: 12))
                .foregroundStyle(.token(hovered ? .foreground : .mutedForeground))
                .underline(hovered)
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .animation(.kz.colors, value: hovered)
    }
}
