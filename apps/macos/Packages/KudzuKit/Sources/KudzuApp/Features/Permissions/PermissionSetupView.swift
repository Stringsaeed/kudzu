import KudzuCore
import KudzuDesign
import SwiftUI

/// Inline permission card (`permission-setup.tsx`). Shown from Settings and onboarding.
public struct PermissionSetupView: View {
    @Environment(AppModel.self) private var model
    private let explicitStatus: EngineStatus?
    @State private var opening = false
    @State private var errorMessage: String?

    public init(status: EngineStatus? = nil) {
        self.explicitStatus = status
    }

    private var status: EngineStatus { explicitStatus ?? model.status }

    public var body: some View {
        let ready = PermissionsLogic.isReady(status)
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                Icon.shieldCheck
                    .icon(20)
                    .foregroundStyle(.kz.irisForeground)
                    .frame(width: 40, height: 40)
                    .background(Color.kz.irisSoft, in: RoundedRectangle(cornerRadius: Radius.xl, style: .continuous))
                VStack(alignment: .leading, spacing: 2) {
                    Text(PermissionsLogic.setupTitle(ready: ready))
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.kz.foreground)
                    Text(PermissionsLogic.setupDetail(ready: ready, status: status))
                        .font(.system(size: 12))
                        .mutedForeground()
                        .fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            HStack(spacing: 6) {
                mark(status.permissions.accessibility, missing: Icon.shieldCheck)
                Text("Accessibility")
                Text("·").foregroundStyle(.kz.faint)
                mark(status.permissions.inputMonitoring, missing: Icon.keyboard)
                Text("Input Monitoring")
            }
            .font(.system(size: 12))
            .mutedForeground()
            .padding(.vertical, 16)

            Button {
                Task { await open() }
            } label: {
                ButtonLabel(PermissionsLogic.setupButton(ready: ready, opening: opening), trailing: Icon.arrowUpRight)
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.kz(ready ? .outline : .default))
            .disabled(opening)

            if let errorMessage {
                Text(errorMessage)
                    .font(.system(size: 12))
                    .foregroundStyle(.kz.destructive)
                    .padding(.top, 12)
            }
        }
        .padding(16)
        .depth(.card, fill: Color.kz.surface, in: RoundedRectangle(cornerRadius: Radius.xl, style: .continuous))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Permissions status")
        // `role="alert"` on the error line.
        .onChange(of: errorMessage) { _, message in
            if let message { AccessibilityNotification.Announcement(message).post() }
        }
    }

    @ViewBuilder
    private func mark(_ granted: Bool, missing: Image) -> some View {
        if granted {
            Icon.check.icon(14).foregroundStyle(.kz.success).checkIn()
        } else {
            missing.icon(14)
        }
    }

    private func open() async {
        opening = true
        errorMessage = nil
        defer { opening = false }
        do {
            try await model.backend.showPermissions()
        } catch {
            errorMessage = AppModel.errorMessage(error)
        }
    }
}
