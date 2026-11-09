import Foundation
import KudzuCore

/// Pure state derivation and copy for `PermissionSetupView` and `PermissionCenterView`
/// (`permission-setup.tsx`, `permission-center.tsx`).
enum PermissionsLogic {
    enum HandoffState: Equatable, Sendable {
        case idle
        case opening
        case waiting
        case error(String)
    }

    /// Accessibility is granted, the app can post events, and the event tap is running.
    static func isReady(_ status: EngineStatus) -> Bool {
        status.permissions.accessibility && status.canPostEvents && status.tap != .off
    }

    /// Accessibility is granted, but expansion is not connected yet. Input Monitoring is not this state.
    static func needsConnection(selected: PermissionKind, status: EngineStatus) -> Bool {
        selected == .accessibility && status.permissions.accessibility && !isReady(status)
    }

    static func selectedGranted(_ status: EngineStatus, selected: PermissionKind) -> Bool {
        switch selected {
        case .accessibility: status.permissions.accessibility
        case .inputMonitoring: status.permissions.inputMonitoring
        }
    }

    static func permissionName(selected: PermissionKind, accessibilityName: String) -> String {
        selected == .accessibility ? accessibilityName : "Input Monitoring"
    }

    static func headline(status: EngineStatus, selected: PermissionKind) -> String {
        if isReady(status) { return "You’re all set." }
        if needsConnection(selected: selected, status: status) { return "Access is allowed." }
        return "Let Kudzu do the typing."
    }

    static func detail(loaded: Bool, status: EngineStatus, selected: PermissionKind) -> String {
        guard loaded else { return "Checking access on this Mac…" }
        if isReady(status) {
            if status.canExpand { return "Your shortcuts can now become full sentences." }
            if status.paused { return "Permissions are set. Expansion is currently paused." }
            return "Permissions are set. Close the secure input field to expand."
        }
        if needsConnection(selected: selected, status: status) {
            return "One last step to connect Kudzu to your keyboard."
        }
        return "A little permission to turn your triggers into text, wherever you write."
    }

    enum PrimaryAction: Equatable, Sendable {
        case backToKudzu
        case restart(appName: String)
        case setupAppControl
        case openSettings(opening: Bool)
    }

    static func primaryAction(
        status: EngineStatus,
        selected: PermissionKind,
        handoff: HandoffState,
        restartOffered: Bool,
        appName: String
    ) -> PrimaryAction {
        let granted = selectedGranted(status, selected: selected)
        if isReady(status), granted { return .backToKudzu }
        if needsConnection(selected: selected, status: status), restartOffered {
            return .restart(appName: appName)
        }
        if selected == .inputMonitoring, granted { return .setupAppControl }
        return .openSettings(opening: handoff == .opening)
    }

    static func primaryLabel(_ action: PrimaryAction) -> String {
        switch action {
        case .backToKudzu: "Back to Kudzu"
        case let .restart(appName): "Restart \(appName)"
        case .setupAppControl: "Set up app control"
        case let .openSettings(opening): opening ? "Opening Settings…" : "Open System Settings"
        }
    }

    /// Trailing label on a permission row. Input Monitoring is optional; Accessibility is required.
    static func rowTrailing(kind: PermissionKind, granted: Bool) -> String {
        if granted { return "Allowed" }
        return kind == .accessibility ? "Required" : "Optional"
    }

    static func setupTitle(ready: Bool) -> String {
        ready ? "Permissions are set" : "Enable typing in other apps"
    }

    static func setupDetail(ready: Bool, status: EngineStatus) -> String {
        guard ready else { return "A guided setup, with every step beside you." }
        return status.canExpand ? "Kudzu is ready to expand your snippets." : status.summary
    }

    static func setupButton(ready: Bool, opening: Bool) -> String {
        if opening { return "Opening…" }
        return ready ? "Review permissions" : "Set up permissions"
    }

    /// The line under "Access confirmed" once the selected permission is on.
    static func grantedFollowUp(
        status: EngineStatus,
        selected: PermissionKind,
        restartOffered: Bool,
        appName: String,
        accessibilityName: String
    ) -> String {
        if isReady(status), selectedGranted(status, selected: selected) {
            return status.canExpand ? "Ready to expand in your other apps." : status.summary
        }
        if needsConnection(selected: selected, status: status) {
            return restartOffered
                ? "macOS may need a restart of \(appName) to apply access. We’ll bring you back here."
                : "Connecting to your keyboard…"
        }
        return "Next, allow \(accessibilityName) so Kudzu can insert text."
    }
}
