import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

/// Companion window for Accessibility and Input Monitoring (`permission-center.tsx`).
public struct PermissionCenterView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @State private var selected: PermissionKind = .accessibility
    @State private var appName = "Kudzu"
    @State private var accessibilityName = "Accessibility"
    @State private var handoff: PermissionsLogic.HandoffState = .idle
    @State private var helpOpen = false
    @State private var dragAvailable = false
    @State private var restartOffered = false
    @State private var loaded = false
    @State private var hoveredRow: PermissionKind?
    @State private var iconHovered = false
    @State private var helpHovered = false

    public init() {}

    private var status: EngineStatus { model.status }
    private var needsConn: Bool { PermissionsLogic.needsConnection(selected: selected, status: status) }
    private var selectedGranted: Bool { PermissionsLogic.selectedGranted(status, selected: selected) }
    private var action: PermissionsLogic.PrimaryAction {
        PermissionsLogic.primaryAction(
            status: status,
            selected: selected,
            handoff: handoff,
            restartOffered: restartOffered,
            appName: appName
        )
    }

    public var body: some View {
        ZStack {
            card
            if helpOpen {
                helpPanel
                    .transition(reduceMotion ? .identity : .opacity.combined(with: .scale(scale: 0.98)))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .animation(reduceMotion ? nil : .easeOut(duration: Motion.fast), value: helpOpen)
        .onKeyPress(.escape) {
            if helpOpen {
                helpOpen = false
            } else {
                Task { await attempt { try await model.backend.closePermissions() } }
            }
            return .handled
        }
        .task { await load() }
        .task(id: needsConn) {
            guard needsConn else {
                restartOffered = false
                return
            }
            try? await Task.sleep(for: .seconds(5))
            if !Task.isCancelled { restartOffered = true }
        }
        .onChange(of: model.permissionKind) { _, kind in
            selected = kind
            handoff = .idle
            helpOpen = false
        }
        // `aria-live="polite"` on the headline.
        .onChange(of: PermissionsLogic.headline(status: status, selected: selected)) { _, headline in
            AccessibilityNotification.Announcement(headline).post()
        }
        .onChange(of: selectedGranted) { _, granted in
            if granted { Task { await attempt { try await model.backend.stopPermissionGuide() } } }
        }
    }

    private var card: some View {
        VStack(spacing: 0) {
            header
            VStack(spacing: 0) {
                iconButton
                    .padding(.top, 12)
                HStack(spacing: 4) {
                    Icon.mousePointer2.icon(12)
                    Text(dragAvailable ? "Drag into System Settings to add the app" : "Your app. Your permission.")
                }
                .font(.system(size: 10))
                .mutedForeground()
                .frame(height: 16)
                .padding(.top, 8)
                Text(PermissionsLogic.headline(status: status, selected: selected))
                    .font(.system(size: 23, weight: .semibold))
                    .tracking(-0.8)
                    .foregroundStyle(.kz.foreground)
                    .multilineTextAlignment(.center)
                    .padding(.top, 20)
                Text(PermissionsLogic.detail(loaded: loaded, status: status, selected: selected))
                    .font(.system(size: 13))
                    .lineSpacing(3)
                    .mutedForeground()
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 310)
                    .padding(.top, 8)

                VStack(spacing: 8) {
                    row(.accessibility, detail: "Insert snippets into other apps.")
                    row(.inputMonitoring, detail: "Additional keyboard monitoring access.")
                }
                .accessibilityElement(children: .contain)
                .accessibilityLabel("Permissions")
                .padding(.top, 24)

                statusLine
                    .frame(minHeight: 72)
                    .padding(.top, 16)

                primaryButton
                    .padding(.top, 8)
                Button {
                    helpOpen.toggle()
                } label: {
                    HStack(spacing: 6) {
                        Icon.circleHelp.icon(14)
                        Text(helpOpen ? "Hide help" : "Don’t see the app, or already allowed?")
                            .font(.system(size: 11))
                    }
                    .foregroundStyle(helpHovered ? Color.kz.foreground : Color.kz.mutedForeground)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 6)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .onHover { helpHovered = $0 }
                .kzAnimation(.kz.hover, value: helpHovered)
                .accessibilityAddTraits(helpOpen ? .isSelected : AccessibilityTraits())

                Text("Your typing is never recorded or sent anywhere.")
                    .font(.system(size: 10))
                    .foregroundStyle(.kz.faint)
                    .padding(.top, 12)
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 20)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            .background(alignment: .top) {
                RadialGradient(
                    colors: [Color.kz.irisSoft, Color.clear],
                    center: .top,
                    startRadius: 0,
                    endRadius: 180
                )
                .frame(height: 176)
                .allowsHitTesting(false)
            }
        }
        // The panel is the card (`vibrant:p-0 vibrant:border-0`): no page behind it, no border.
        .frame(maxWidth: 420, maxHeight: .infinity)
        .background(Color.kz.popover, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .depth(.raised, fill: Color.clear, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
    }

    private var header: some View {
        HStack(spacing: 8) {
            Icon.shieldCheck.icon(14)
            Text("Permissions for \(appName)")
                .font(.system(size: 12))
                .frame(maxWidth: .infinity, alignment: .leading)
            Button {
                Task { await attempt { try await model.backend.closePermissions() } }
            } label: {
                Icon.x.icon(14)
            }
            .buttonStyle(.kz(.ghost, size: .iconXs))
            .accessibilityLabel("Close permission setup")
        }
        .foregroundStyle(.kz.mutedForeground)
        .padding(.horizontal, 20)
        .frame(height: 48)
    }

    private var iconButton: some View {
        Button {
            Task { await attempt { try await model.backend.revealPermissionApp() } }
        } label: {
            Group {
                if let icon = NSApp?.applicationIconImage {
                    Image(nsImage: icon)
                        .resizable()
                        .interpolation(.high)
                        .frame(width: 80, height: 80)
                } else {
                    Wordmark(.small)
                        .frame(width: 64, height: 64)
                }
            }
            .frame(width: 80, height: 80)
            // `transition-transform hover:scale-105 cursor-grab`.
            .scaleEffect(iconHovered ? 1.05 : 1)
            .kzAnimation(.kz.ease(duration: 0.15), value: iconHovered)
        }
        .buttonStyle(.plain)
        .onHover { inside in
            guard inside != iconHovered else { return }
            iconHovered = inside
            if inside { NSCursor.openHand.push() } else { NSCursor.pop() }
        }
        .help("Drag this app into System Settings")
        .accessibilityLabel("Show \(appName) in Finder to add it to System Settings")
        .background {
            GeometryReader { geo in
                Color.clear
                    .onAppear { reportIconBounds(geo.frame(in: .global)) }
                    .onChange(of: geo.frame(in: .global)) { _, frame in reportIconBounds(frame) }
            }
        }
    }

    private func row(_ kind: PermissionKind, detail: String) -> some View {
        let granted = PermissionsLogic.selectedGranted(status, selected: kind)
        let title = PermissionsLogic.permissionName(selected: kind, accessibilityName: accessibilityName)
        let on = selected == kind
        return Button {
            choose(kind)
        } label: {
            HStack(spacing: 12) {
                iconWell(kind: kind, granted: granted)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.system(size: 13))
                        .foregroundStyle(.kz.foreground)
                    Text(detail)
                        .font(.system(size: 11))
                        .mutedForeground()
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                HStack(spacing: 4) {
                    if granted { Icon.check.icon(14).foregroundStyle(.kz.success).checkIn() }
                    Text(PermissionsLogic.rowTrailing(kind: kind, granted: granted))
                        .font(.system(size: 10, weight: .medium))
                }
                .foregroundStyle(granted ? Color.kz.success : Color.kz.mutedForeground)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 12)
            // Selected `border-iris/40 bg-iris-soft/50 shadow-card`; otherwise `border-hairline
            // bg-surface/50 hover:bg-surface` (Item's `transition-colors duration-100`).
            .depth(
                on ? .card : .none,
                fill: on ? Color.kz.mix(.irisSoft, alpha: 0.5) : (hoveredRow == kind ? Color.kz.surface : Color.kz.mix(.surface, alpha: 0.5)),
                in: RoundedRectangle(cornerRadius: Radius.xl, style: .continuous)
            )
            .overlay {
                RoundedRectangle(cornerRadius: Radius.xl, style: .continuous)
                    .strokeBorder(on ? Color.kz.mix(.iris, alpha: 0.4) : Color.kz.hairline, lineWidth: 1)
            }
            .contentShape(RoundedRectangle(cornerRadius: Radius.xl, style: .continuous))
            .kzAnimation(.kz.ease(duration: 0.1), value: hoveredRow == kind)
            .kzAnimation(.kz.ease(duration: 0.1), value: on)
        }
        .buttonStyle(.plain)
        .onHover { inside in
            if inside { hoveredRow = kind } else if hoveredRow == kind { hoveredRow = nil }
        }
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    private func iconWell(kind: PermissionKind, granted: Bool) -> some View {
        let symbol = kind == .accessibility ? Icon.shieldCheck : Icon.keyboard
        return symbol
            .icon(16)
            .foregroundStyle(granted ? Color.kz.success : Color.kz.mutedForeground)
            .frame(width: 32, height: 32)
            .background(
                granted ? Color.kz.mix(.success, alpha: 0.1) : Color.kz.mix(.foreground, alpha: 0.05),
                in: RoundedRectangle(cornerRadius: Radius.lg, style: .continuous)
            )
    }

    @ViewBuilder
    private var statusLine: some View {
        if case let .error(message) = handoff {
            Text(message)
                .font(.system(size: 12))
                .foregroundStyle(.kz.destructive)
                .padding(12)
                .frame(maxWidth: .infinity)
                .background(Color.kz.mix(.destructive, alpha: 0.1), in: RoundedRectangle(cornerRadius: Radius.lg, style: .continuous))
        } else if selectedGranted {
            VStack(spacing: 8) {
                HStack(spacing: 6) {
                    Icon.check.icon(16).foregroundStyle(.kz.success).checkIn()
                    Text("Access confirmed")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(.kz.success)
                }
                Text(
                    PermissionsLogic.grantedFollowUp(
                        status: status,
                        selected: selected,
                        restartOffered: restartOffered,
                        appName: appName,
                        accessibilityName: accessibilityName
                    )
                )
                .font(.system(size: 11))
                .mutedForeground()
                .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .permissionConfirm()
        } else if handoff == .waiting {
            VStack(spacing: 8) {
                HStack(spacing: 6) {
                    ProgressView().controlSize(.small)
                    Text("Waiting for macOS")
                        .font(.system(size: 12, weight: .medium))
                }
                let name = Text(appName).fontWeight(.medium).foregroundStyle(Color.kz.foreground)
                let permission = PermissionsLogic.permissionName(selected: selected, accessibilityName: accessibilityName)
                Text("Turn on \(name) in \(permission).\nIf it isn’t listed, drag the icon into the list.")
                    .font(.system(size: 11))
                    .lineSpacing(3)
                    .mutedForeground()
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
        } else {
            Text("We’ll open the right page and stay beside you. macOS may ask you to confirm with Touch ID or your password.")
                .font(.system(size: 11))
                .lineSpacing(2)
                .mutedForeground()
                .multilineTextAlignment(.center)
                .padding(.horizontal, 12)
        }
    }

    private var primaryButton: some View {
        Button {
            Task { await perform(action) }
        } label: {
            HStack(spacing: 6) {
                Text(PermissionsLogic.primaryLabel(action))
                switch action {
                case .backToKudzu:
                    Icon.check.icon(16)
                case .restart:
                    EmptyView()
                case .setupAppControl, .openSettings:
                    Icon.arrowUpRight.icon(16)
                }
            }
            .frame(maxWidth: .infinity)
        }
        .buttonStyle(.kz(.default, size: .xl))
        .disabled(openSettingsDisabled)
    }

    private var openSettingsDisabled: Bool {
        if case .openSettings = action { return !loaded || handoff == .opening }
        return false
    }

    private var helpPanel: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Add \(appName) to the list")
                    .font(.system(size: 13, weight: .semibold))
                Spacer()
                Button {
                    helpOpen = false
                } label: {
                    Icon.x.icon(14)
                }
                .buttonStyle(.kz(.ghost, size: .iconXs))
                .accessibilityLabel("Close help")
            }
            VStack(alignment: .leading, spacing: 8) {
                Text("1. Open Privacy & Security → \(PermissionsLogic.permissionName(selected: selected, accessibilityName: accessibilityName)).")
                Text("2. Drag the app icon into the list, or use the + button to select \(appName).")
                Text("3. Turn on its switch and approve the macOS prompt.")
            }
            .font(.system(size: 12))
            .mutedForeground()
            Text("Already on? Quit and reopen \(appName) if macOS hasn’t applied the change. Make sure you enabled this copy of the app.")
                .font(.system(size: 12))
                .mutedForeground()
                .padding(.top, 4)
            Button {
                Task { await attempt { try await model.backend.revealPermissionApp() } }
            } label: {
                ButtonLabel("Show this app in Finder", icon: Icon.folderOpen)
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.kz(.outline, size: .sm))
            Button("Restart \(appName)") {
                Task { await attempt { try await model.backend.restartPermissionApp() } }
            }
            .buttonStyle(.kz(.ghost, size: .sm))
            .frame(maxWidth: .infinity)
        }
        .padding(16)
        .depth(.raised, fill: Color.kz.popover, in: RoundedRectangle(cornerRadius: Radius.xl, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: Radius.xl, style: .continuous)
                .strokeBorder(Color.kz.hairline, lineWidth: 1)
        }
        .padding(16)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
    }

    private func load() async {
        await attempt {
            let context = try await model.backend.permissionContext()
            selected = context.kind
            appName = context.appName
            accessibilityName = context.accessibilityName
        }
        while !Task.isCancelled {
            await attempt { model.status = try await model.backend.engineStatus() }
            loaded = true
            try? await Task.sleep(for: .seconds(1))
        }
    }

    private func reportIconBounds(_ frame: CGRect) {
        guard frame.width >= 1, frame.height >= 1 else { return }
        let rect = ScreenRect(
            x: Double(frame.minX.rounded()),
            y: Double(frame.minY.rounded()),
            width: Double(frame.width.rounded()),
            height: Double(frame.height.rounded())
        )
        Task {
            await attempt { dragAvailable = try await model.backend.installPermissionIcon(bounds: rect) }
        }
    }

    /// The web's `fail`: any backend failure shows in the handoff area instead of vanishing.
    private func attempt(_ work: () async throws -> Void) async {
        do {
            try await work()
        } catch {
            handoff = .error(AppModel.errorMessage(error))
        }
    }

    private func choose(_ kind: PermissionKind) {
        selected = kind
        handoff = .idle
        Task { await attempt { try await model.backend.stopPermissionGuide() } }
    }

    private func perform(_ action: PermissionsLogic.PrimaryAction) async {
        switch action {
        case .backToKudzu:
            await attempt { try await model.backend.closePermissions() }
        case .restart:
            await attempt { try await model.backend.restartPermissionApp() }
        case .setupAppControl:
            choose(.accessibility)
        case .openSettings:
            handoff = .opening
            do {
                try await model.backend.beginPermission(kind: selected)
                handoff = .waiting
            } catch {
                handoff = .error(AppModel.errorMessage(error))
            }
        }
    }
}

#Preview("Permission Center") {
    let model = AppModel(backend: MockBackend(), isDev: true)
    PermissionCenterView()
        .environment(model)
        .frame(width: 460, height: 700)
        .task { await model.start() }
}
