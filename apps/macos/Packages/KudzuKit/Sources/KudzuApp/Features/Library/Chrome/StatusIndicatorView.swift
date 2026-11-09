// Engine health (`status.tsx`): a dot and a label, plus pause. Blocked and limited open Settings.

import KudzuCore
import KudzuDesign
import SwiftUI

struct StatusIndicatorView: View {
    @Environment(AppModel.self) private var model
    @Environment(LibraryActions.self) private var actions

    var body: some View {
        let health = EngineHealth(status: model.status, platform: model.platform)
        HStack(spacing: 4) {
            Button {
                if health.opensSettings { model.setView(.settings(section: .general)) }
            } label: {
                HStack(spacing: 8) {
                    Circle()
                        .fill(dot(health))
                        .frame(width: 6, height: 6)
                        .shadow(color: health == .ready ? Color.kz.success : .clear, radius: 4)
                    Text(health.label)
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(Color.kz.sidebarForeground.opacity(0.8))
                        .lineLimit(1)
                    if health.showsShield {
                        Icon.shieldAlert.icon(14).foregroundStyle(Color.kz.warning)
                    }
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
            }
            .buttonStyle(SidebarHoverStyle())
            .accessibilityLabel("Status: \(health.label). \(health.detail(owner: model.status.secureInputOwner))")
            .help("\(health.detail(owner: model.status.secureInputOwner))\nQuick search anywhere: \(Keys.acceleratorLabels(model.settings.paletteHotkey).joined())")
            Button { actions.togglePaused() } label: {
                (model.status.paused ? Icon.play : Icon.pause).icon(14)
                    .frame(width: 28, height: 28)
            }
            .buttonStyle(SidebarHoverStyle(dimmed: true, press: true))
            .accessibilityLabel(model.status.paused ? "Resume expansion" : "Pause expansion")
            .accessibilityValue(model.status.paused ? "paused" : "running")
            .kzHelp(model.status.paused ? "Resume" : "Pause", shortcut: "mod+shift+p")
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .overlay(alignment: .top) { Hairline() }
    }

    private func dot(_ health: EngineHealth) -> Color {
        switch health {
        case .ready: Color.kz.success
        case .blocked: Color.kz.destructive
        case .paused, .limited, .secure: Color.kz.warning
        }
    }
}

/// The status row's buttons: `rounded-md hover:bg-sidebar-accent`; the pause button also starts
/// muted and turns `hover:text-foreground`, with `press`.
private struct SidebarHoverStyle: ButtonStyle {
    var dimmed = false
    var press = false

    func makeBody(configuration: Configuration) -> some View {
        HoverBody(configuration: configuration, dimmed: dimmed, press: press)
    }

    private struct HoverBody: View {
        var configuration: Configuration
        var dimmed: Bool
        var press: Bool
        @State private var hovered = false

        var body: some View {
            let shape = RoundedRectangle(cornerRadius: Radius.md, style: .continuous)
            configuration.label
                .foregroundStyle(dimmed && !hovered ? Color.kz.mutedForeground : Color.kz.foreground)
                .background(hovered ? Color.kz.sidebarAccent : Color.clear, in: shape)
                .contentShape(shape)
                .onHover { hovered = $0 }
                .themeAwareAnimation(.kz.hover, value: hovered)
                .press(press && configuration.isPressed)
        }
    }
}
