import AppKit
import KudzuCore
import SwiftUI
import Testing
@testable import KudzuApp

/// Hosted captures of the settings pages for review. Runs only with `KZ_CAPTURE_DIR` set.
@MainActor
@Suite struct SettingsCaptureTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["KZ_CAPTURE_DIR"] != nil))
    func captureSettingsPages() async throws {
        let dir = URL(fileURLWithPath: ProcessInfo.processInfo.environment["KZ_CAPTURE_DIR"]!)
        let model = AppModel(backend: MockBackend(), isDev: true)
        await model.start()
        for section in [SettingsSection.general, .clipboard, .agents, .data, .about] {
            let view = SettingsScreen(section: section)
                .environment(model)
                .frame(width: 920, height: 1100)
            let host = NSHostingView(rootView: view)
            host.frame = NSRect(x: 0, y: 0, width: 920, height: 1100)
            host.appearance = NSAppearance(named: .aqua)
            let window = NSWindow(contentRect: host.frame, styleMask: [.borderless], backing: .buffered, defer: false)
            window.contentView = host
            host.layoutSubtreeIfNeeded()
            try await Task.sleep(for: .milliseconds(600))
            host.layoutSubtreeIfNeeded()
            let rep = try #require(host.bitmapImageRepForCachingDisplay(in: host.bounds))
            host.cacheDisplay(in: host.bounds, to: rep)
            let png = try #require(rep.representation(using: .png, properties: [:]))
            try png.write(to: dir.appendingPathComponent("settings-\(section.rawValue).png"))
        }
    }
}
