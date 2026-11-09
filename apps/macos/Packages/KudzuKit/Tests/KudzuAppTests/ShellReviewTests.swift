import AppKit
import KudzuCore
import Testing
@testable import KudzuApp

@MainActor @Suite struct ShellReviewTests {
    @Test func shutdownKeepsItsClaimUntilTheAcceptWorkerFinishes() async throws {
        let path = "/private/tmp/kz-stop-\(UUID().uuidString.prefix(12)).sock"
        defer { try? FileManager.default.removeItem(atPath: path + ".lock") }
        let entered = DispatchSemaphore(value: 0)
        let release = DispatchSemaphore(value: 0)
        let oldCalls = SocketCallCount()
        let newCalls = SocketCallCount()
        let old = try #require(InstanceSocket.listen(path) {
            oldCalls.increment()
            entered.signal()
            release.wait()
        })
        defer { release.signal(); old.close() }
        #expect(InstanceSocket.notify(path) != nil)
        let didEnter = await withCheckedContinuation { continuation in
            DispatchQueue.global().async { continuation.resume(returning: entered.wait(timeout: .now() + 2) == .success) }
        }
        #expect(didEnter)
        old.close()
        old.close()
        let premature = InstanceSocket.listen(path) {}
        #expect(premature == nil)
        premature?.close()
        await premature?.waitForShutdown()
        release.signal()
        await old.waitForShutdown()
        let replacement = try #require(InstanceSocket.listen(path) { newCalls.increment() })
        #expect(InstanceSocket.notify(path) != nil)
        for _ in 0..<200 {
            if newCalls.value == 1 { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        replacement.close()
        replacement.close()
        await replacement.waitForShutdown()
        #expect(oldCalls.value == 1 && newCalls.value == 1)
    }

    @Test func shutdownWakesBlockedAcceptAndReleasesOwnership() async throws {
        let path = "/private/tmp/kz-idle-\(UUID().uuidString.prefix(12)).sock"
        defer { try? FileManager.default.removeItem(atPath: path + ".lock") }
        let socket = try #require(InstanceSocket.listen(path) {})
        socket.close()
        socket.close()
        await socket.waitForShutdown()
        let replacement = try #require(InstanceSocket.listen(path) {})
        replacement.close()
        await replacement.waitForShutdown()
    }

    @Test func simultaneousSocketClaimsAllowOneNativeOwner() async throws {
        let path = "/private/tmp/kz-claim-\(UUID().uuidString.prefix(12)).sock"
        defer { try? FileManager.default.removeItem(atPath: path + ".lock") }
        let owners = await withTaskGroup(of: InstanceSocket?.self, returning: [InstanceSocket].self) { group in
            for _ in 0..<24 { group.addTask { InstanceSocket.listen(path) {} } }
            var owners: [InstanceSocket] = []
            for await owner in group { if let owner { owners.append(owner) } }
            return owners
        }
        defer { owners.forEach { $0.close() } }
        #expect(owners.count == 1)
        #expect(InstanceSocket.notify(path) == ProcessInfo.processInfo.processIdentifier)
        #expect(InstanceSocket.listen(path) {} == nil)
        owners.forEach { $0.close() }
        for owner in owners { await owner.waitForShutdown() }
    }

    @Test func unexpectedSocketPathFailsClosedWithoutReplacingIt() throws {
        let path = "/private/tmp/kz-path-\(UUID().uuidString.prefix(12)).sock"
        let original = Data("keep this file".utf8)
        try original.write(to: URL(fileURLWithPath: path))
        defer {
            try? FileManager.default.removeItem(atPath: path)
            try? FileManager.default.removeItem(atPath: path + ".lock")
        }
        #expect(InstanceSocket.listen(path) {} == nil)
        #expect(try Data(contentsOf: URL(fileURLWithPath: path)) == original)
    }

    @Test func reopeningPreservesReachableWindowsAndRecoversRemovedDisplays() {
        let primary = NSRect(x: 0, y: 0, width: 1440, height: 900)
        let left = NSRect(x: -1920, y: -180, width: 1920, height: 1080)
        let onLeft = NSRect(x: -1600, y: 20, width: 1180, height: 760)
        #expect(WindowPlacement.recover(onLeft, screens: [primary, left]) == onLeft)
        #expect(WindowPlacement.recover(onLeft, screens: [primary]) == NSRect(x: 0, y: 20, width: 1180, height: 760))

        let partlyVisible = NSRect(x: 1300, y: 20, width: 1180, height: 760)
        #expect(WindowPlacement.recover(partlyVisible, screens: [primary]) == partlyVisible)
        let aboveMenuBar = NSRect(x: 20, y: 700, width: 1180, height: 760)
        #expect(WindowPlacement.recover(aboveMenuBar, screens: [primary]) == NSRect(x: 20, y: 140, width: 1180, height: 760))
    }

    @Test func recoveryKeepsMinimumWindowSizeOnSmallScreens() {
        let frame = NSRect(x: 2000, y: 0, width: 900, height: 560)
        let small = NSRect(x: -640, y: 0, width: 640, height: 480)
        let recovered = WindowPlacement.recover(frame, screens: [small])
        #expect(recovered == NSRect(x: -640, y: -80, width: 900, height: 560))
        #expect(WindowPlacement.recover(recovered, screens: [small]) == recovered)
        #expect(WindowPlacement.recover(frame, screens: []) == frame)
    }

    @Test func nativeQuitRequestsTerminateWithoutAnExplicitMenuFlag() {
        let delegate = AppDelegate(launchedHidden: true)
        #expect(delegate.applicationShouldTerminate(NSApplication.shared) == .terminateNow)
        #expect(!delegate.applicationShouldTerminateAfterLastWindowClosed(NSApplication.shared))
    }

    @Test func panelsUseTheTauriPositionsAndCaretClamping() {
        let monitor = PanelPlacement.Monitor(x: -1920, y: -200, w: 1920, h: 1080)
        let palette = PanelPlacement.centered(width: 760, on: monitor, topFraction: 0.2)
        #expect(palette.x == -1340 && palette.y == 16)
        let clipboard = PanelPlacement.centered(width: 800, on: monitor, topFraction: 0.18)
        #expect(clipboard.x == -1360 && abs(clipboard.y - (-5.6)) < 0.0001)
        let form = PanelPlacement.form(caret: ScreenRect(x: -10, y: 800, width: 1, height: 20), monitor: monitor, height: 360)
        #expect(form.x == -448 && form.y == 434)
        let fallback = PanelPlacement.form(caret: nil, monitor: monitor, height: 360)
        #expect(fallback.x == -1180 && abs(fallback.y - 102.4) < 0.0001)
    }

    @Test func secondLaunchDuringStartupWaitsForTheShell() {
        let instance = SingleInstance(bundleID: "sh.saeed.kudzu.test")
        instance.requestShowMain()
        instance.requestShowMain()
        var shown = 0
        instance.onShowMain = { shown += 1 }
        #expect(shown == 1)
        instance.requestShowMain()
        #expect(shown == 2)
    }

    @Test func statusGlyphsAndMenuSummaryFollowTauriPriority() {
        var status = EngineStatus(canPostEvents: true, canExpand: true, tap: .active,
                                  permissions: .init(accessibility: true, inputMonitoring: true), summary: "Ready")
        #expect(StatusItemController.glyph(for: status).rawValue == "MenuBar")
        #expect(StatusItemController.summary(status) == "Expanding")
        status.secureInput = true
        status.secureInputOwner = "Terminal"
        #expect(StatusItemController.glyph(for: status).rawValue == "MenuBarSecureInput")
        #expect(StatusItemController.summary(status) == "Blocked — Secure Input in Terminal")
        status.paused = true
        #expect(StatusItemController.glyph(for: status).rawValue == "MenuBarPaused")
        #expect(StatusItemController.summary(status) == "Paused")
        status.paused = false
        status.secureInput = false
        status.permissions.accessibility = false
        #expect(StatusItemController.glyph(for: status).rawValue == "MenuBarPermissions")
    }
}

private final class SocketCallCount: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0
    var value: Int { lock.withLock { count } }
    func increment() { lock.withLock { count += 1 } }
}
