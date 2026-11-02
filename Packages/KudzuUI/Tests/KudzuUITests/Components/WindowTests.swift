import AppKit
@testable import KudzuUI
import SwiftUI
import Testing

@MainActor
@Suite struct WindowChromeTests {
    private func makeWindow() -> NSWindow {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 900, height: 600),
                              styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.contentView = NSView(frame: NSRect(x: 0, y: 0, width: 900, height: 600))
        return window
    }

    /// The numbers in `KzWindowChrome`'s documentation: the chrome lays out a 52pt titlebar whose
    /// traffic lights are centred on y = 26, exactly the middle of a `size-titlebar` header.
    @Test func trafficLightsCentreOnTheHeader() throws {
        let window = makeWindow()
        // Without the chrome the lights sit 16pt from the top, 10pt above the header's centre.
        window.styleMask.insert(.fullSizeContentView)
        window.titlebarAppearsTransparent = true
        window.layoutIfNeeded()
        let before = try #require(KzWindowChrome.closeButtonCenterY(in: window))
        #expect(abs(before - 16) < 0.5)

        KzWindowChrome.apply(to: makeWindowReusing(window))
        window.layoutIfNeeded()
        let after = try #require(KzWindowChrome.closeButtonCenterY(in: window))
        #expect(abs(after - KzSize.titlebar / 2) < 0.5, "lights centre at \(after), header centre at \(KzSize.titlebar / 2)")
        #expect(abs(after - KzWindowChrome.Lights.centerY) < 0.5)

        // Only the buttons move: a taller titlebar would be drawn as an opaque strip over the headers.
        let button = try #require(window.standardWindowButton(.closeButton))
        let container = try #require(button.superview?.superview)
        #expect(container.frame.height < KzSize.titlebar, "titlebar is \(container.frame.height)pt")
    }

    private func makeWindowReusing(_ window: NSWindow) -> NSWindow { window }

    @Test func chromeSettings() {
        let window = makeWindow()
        window.level = .floating
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        KzWindowChrome.apply(to: window)
        #expect(window.titleVisibility == .hidden)
        #expect(window.titlebarAppearsTransparent)
        #expect(window.styleMask.contains(.fullSizeContentView))
        // No toolbar: on macOS 26 it would draw glass over the pane headers.
        #expect(window.toolbar == nil)
        #expect(window.isOpaque)
        #expect(window.backgroundColor == KzColorToken.surfaceRaised.nsColor)
        #expect(window.standardWindowButton(.closeButton)?.isHidden == false)
        #expect(window.level == .normal)
        #expect(window.styleMask.contains(.resizable))
        #expect(window.collectionBehavior.contains(.fullScreenPrimary))
        #expect(!window.collectionBehavior.contains(.canJoinAllSpaces))
        #expect(!window.collectionBehavior.contains(.fullScreenAuxiliary))

        // Idempotent, and an app's own toolbar is respected.
        let own = NSToolbar(identifier: "app")
        window.toolbar = own
        KzWindowChrome.apply(to: window)
        #expect(window.toolbar === own)
    }

    /// AppKit redoes the titlebar on resize; the lights must end up centred on the header again.
    @Test func lightsSurviveAResize() throws {
        let window = makeWindow()
        KzWindowChrome.apply(to: window)
        window.setFrame(NSRect(x: 0, y: 0, width: 1200, height: 800), display: false)
        KzWindowChrome.layoutTrafficLights(in: window)
        window.layoutIfNeeded()
        let center = try #require(KzWindowChrome.closeButtonCenterY(in: window))
        #expect(abs(center - KzSize.titlebar / 2) < 0.5, "lights centre at \(center)")
        let close = try #require(window.standardWindowButton(.closeButton)).convert(.zero, to: nil)
        #expect(abs(close.x - KzWindowChrome.Lights.leading) < 0.5)
    }

    @Test func shellAppliesChromeToItsWindow() async throws {
        let shell = KzAppShell(sidebar: { Text("s") }, sidebarFooter: { Text("f") }, list: { Text("l") }, detail: { Text("d") })
        let host = NSHostingView(rootView: shell)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1000, height: 600),
                              styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.contentView = host
        // SwiftUI creates the chrome probe a beat after the view is installed; poll rather than guess a delay.
        for _ in 0..<40 where window.titleVisibility != .hidden {
            host.layoutSubtreeIfNeeded()
            await settle(0.05)
        }
        // The chrome lays the lights out again on the next main-loop turn, after SwiftUI's own setup.
        for _ in 0..<20 where abs((KzWindowChrome.closeButtonCenterY(in: window) ?? 0) - KzSize.titlebar / 2) >= 0.5 {
            await settle(0.05)
        }
        #expect(window.titleVisibility == .hidden)
        #expect(window.isOpaque)
        #expect(window.toolbar == nil)
        let center = try #require(KzWindowChrome.closeButtonCenterY(in: window))
        #expect(abs(center - KzSize.titlebar / 2) < 0.5, "centre \(center)")

        window.collectionBehavior = [.fullScreenNone, .canJoinAllSpaces]
        #expect(window.collectionBehavior.contains(.fullScreenPrimary))
        #expect(!window.collectionBehavior.contains(.fullScreenNone))
        #expect(!window.collectionBehavior.contains(.canJoinAllSpaces))

        let close = try #require(window.standardWindowButton(.closeButton))
        close.setFrameOrigin(NSPoint(x: close.frame.origin.x, y: close.frame.origin.y + 10))
        let correctedCenter = try #require(KzWindowChrome.closeButtonCenterY(in: window))
        #expect(abs(correctedCenter - KzSize.titlebar / 2) < 0.5)
    }
}
