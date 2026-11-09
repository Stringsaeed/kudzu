import KudzuCore
@testable import KudzuEngine
import Testing

// Port of the tests in `kudzu-engine/src/status.rs`.
@Suite struct StatusTests {
    @Test func allPermissionPauseAndSecureInputCombinations() {
        for tap in [TapMode.off, .listenOnly, .active] {
            for bits in 0..<64 {
                let accessibility = bits & 1 != 0
                let monitoring = bits & 2 != 0
                let post = bits & 4 != 0
                let paused = bits & 8 != 0
                let secure = bits & 16 != 0
                let owner: String? = bits & 32 != 0 ? "Terminal" : nil
                let result = EngineStatusBuilder.build(StatusInputs(paused: paused, tap: tap,
                    permissions: PermissionStatus(accessibility: accessibility, inputMonitoring: monitoring),
                    postEvent: post, secureInput: secure, secureInputOwner: owner))
                let summary: String
                if paused { summary = "Paused" }
                else if secure { summary = owner.map { "Secure Input on (\($0)) — expansion blocked" } ?? "Secure Input on — expansion blocked" }
                else if tap == .off { summary = accessibility || monitoring ? "Keyboard monitoring unavailable" : "Needs Accessibility" }
                else if !post { summary = "Needs Accessibility to type" }
                else { summary = tap == .active ? "Ready" : "Ready (limited — grant Accessibility for best reliability)" }
                #expect(result.summary == summary)
                #expect(result.canPostEvents == post)
                #expect(result.canExpand == (tap != .off && post && !paused && !secure))
                #expect(result.secureInputOwner == (secure ? owner : nil))
            }
        }
    }

    private func inputs(
        paused: Bool = false,
        tap: TapMode = .active,
        permissions: PermissionStatus = PermissionStatus(accessibility: true, inputMonitoring: true),
        postEvent: Bool = true,
        secureInput: Bool = false,
        secureInputOwner: String? = nil
    ) -> StatusInputs {
        StatusInputs(paused: paused, tap: tap, permissions: permissions, postEvent: postEvent, secureInput: secureInput, secureInputOwner: secureInputOwner)
    }

    private func build(_ inputs: StatusInputs) -> EngineStatus { EngineStatusBuilder.build(inputs) }

    @Test func readyWhenActive() {
        #expect(build(inputs()).summary == "Ready")
    }

    @Test func readinessRequiresAListenerAndPermissionToInsert() {
        #expect(build(inputs()).canExpand)
        #expect(!build(inputs(tap: .off)).canExpand)
        let listeningOnly = build(inputs(tap: .listenOnly, postEvent: false))
        #expect(!listeningOnly.canExpand)
        #expect(!listeningOnly.canPostEvents)
        #expect(build(inputs(tap: .listenOnly)).canExpand)
    }

    @Test func grantsDoNotOverridePauseOrSecureInput() {
        #expect(!build(inputs(paused: true)).canExpand)
        #expect(!build(inputs(secureInput: true)).canExpand)
        #expect(build(inputs(paused: true)).canPostEvents)
    }

    @Test func pausedWins() {
        let status = build(inputs(paused: true, secureInput: true))
        #expect(status.summary == "Paused")
        #expect(status.paused)
    }

    @Test func secureInputNamesOwner() {
        let status = build(inputs(secureInput: true, secureInputOwner: "Terminal"))
        #expect(status.summary == "Secure Input on (Terminal) — expansion blocked")
        #expect(status.secureInputOwner == "Terminal")
        #expect(build(inputs(secureInput: true)).summary == "Secure Input on — expansion blocked")
    }

    @Test func ownerDroppedWhenSecureInputOff() {
        #expect(build(inputs(secureInputOwner: "X")).secureInputOwner == nil)
    }

    @Test func missingPermissions() {
        #expect(build(inputs(tap: .off, permissions: PermissionStatus(), postEvent: false)).summary == "Needs Accessibility")
        #expect(build(inputs(tap: .off)).summary == "Keyboard monitoring unavailable")
        let limited = inputs(tap: .listenOnly, permissions: PermissionStatus(accessibility: false, inputMonitoring: true), postEvent: false)
        #expect(build(limited).summary == "Needs Accessibility to type")
    }

    @Test func listenOnlyIsLimited() {
        #expect(build(inputs(tap: .listenOnly)).summary.hasPrefix("Ready (limited"))
    }
}
