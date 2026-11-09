import KudzuCore

// Engine status and its one-line summary. Port of `kudzu-engine/src/status.rs`.

/// Inputs to ``EngineStatusBuilder/build(_:)``.
struct StatusInputs: Hashable, Sendable {
    var paused: Bool
    var tap: TapMode
    var permissions: PermissionStatus
    /// `CGPreflightPostEventAccess`: needed to type the expansion.
    var postEvent: Bool
    var secureInput: Bool
    var secureInputOwner: String?
}

enum EngineStatusBuilder {
    /// Builds the status with its summary.
    static func build(_ inputs: StatusInputs) -> EngineStatus {
        EngineStatus(
            canPostEvents: inputs.postEvent,
            canExpand: inputs.tap != .off && inputs.postEvent && !inputs.paused && !inputs.secureInput,
            paused: inputs.paused,
            tap: inputs.tap,
            permissions: inputs.permissions,
            secureInput: inputs.secureInput,
            secureInputOwner: inputs.secureInput ? inputs.secureInputOwner : nil,
            summary: summary(inputs)
        )
    }

    private static func summary(_ s: StatusInputs) -> String {
        if s.paused {
            return "Paused"
        }
        if s.secureInput {
            if let owner = s.secureInputOwner {
                return "Secure Input on (\(owner)) — expansion blocked"
            }
            return "Secure Input on — expansion blocked"
        }
        switch s.tap {
        case .off where !s.permissions.accessibility && !s.permissions.inputMonitoring:
            return "Needs Accessibility"
        case .off:
            return "Keyboard monitoring unavailable"
        case _ where !s.postEvent:
            return "Needs Accessibility to type"
        case .listenOnly:
            return "Ready (limited — grant Accessibility for best reliability)"
        case .active:
            return "Ready"
        }
    }
}
