import AppKit
import ApplicationServices
import Observation

@MainActor @Observable
final class Permissions {
    enum Kind: String, CaseIterable, Identifiable {
        case accessibility = "Accessibility"
        case inputMonitoring = "Input Monitoring"
        case postEvent = "Post Event"
        var id: Self { self }
        var pane: String { self == .inputMonitoring ? "Privacy_ListenEvent" : "Privacy_Accessibility" }
    }

    private(set) var accessibility = false
    private(set) var inputMonitoring = false
    private(set) var postEvent = false

    init() { refresh() }

    func refresh() {
        accessibility = AXIsProcessTrusted()
        inputMonitoring = CGPreflightListenEventAccess()
        postEvent = CGPreflightPostEventAccess()
    }

    func granted(_ kind: Kind) -> Bool {
        switch kind {
        case .accessibility: accessibility
        case .inputMonitoring: inputMonitoring
        case .postEvent: postEvent
        }
    }

    func request(_ kind: Kind) {
        switch kind {
        case .accessibility:
            // The SDK imports the constant as mutable global state under Swift 6.
            let key = "AXTrustedCheckOptionPrompt"
            _ = AXIsProcessTrustedWithOptions([key: true] as CFDictionary)
        case .inputMonitoring: _ = CGRequestListenEventAccess()
        case .postEvent: _ = CGRequestPostEventAccess()
        }
        refresh()
        if !granted(kind) { openSettings(kind) }
    }

    func openSettings(_ kind: Kind) {
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?\(kind.pane)") {
            NSWorkspace.shared.open(url)
        }
    }
}
