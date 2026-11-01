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

    /// The numbers in `KzWindowChrome`'s documentation: an empty unified toolbar gives a 52pt titlebar
    /// whose traffic lights are centred on y = 26, exactly the middle of a `size-titlebar` header.
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

        let button = try #require(window.standardWindowButton(.closeButton))
        let container = try #require(button.superview?.superview)
        #expect(container.frame.height == KzSize.titlebar, "titlebar is \(container.frame.height)pt")
    }

    private func makeWindowReusing(_ window: NSWindow) -> NSWindow { window }

    @Test func chromeSettings() {
        let window = makeWindow()
        KzWindowChrome.apply(to: window)
        #expect(window.titleVisibility == .hidden)
        #expect(window.titlebarAppearsTransparent)
        #expect(window.styleMask.contains(.fullSizeContentView))
        #expect(window.toolbar?.identifier == KzWindowChrome.toolbarIdentifier)
        #expect(window.toolbar?.items.isEmpty == true)
        #expect(window.isOpaque == false)
        #expect(window.backgroundColor == .clear)
        #expect(window.standardWindowButton(.closeButton)?.isHidden == false)

        // Idempotent, and an app's own toolbar is respected.
        let toolbar = window.toolbar
        KzWindowChrome.apply(to: window)
        #expect(window.toolbar === toolbar)
    }

    @Test func shellAppliesChromeToItsWindow() async throws {
        let shell = KzAppShell(sidebar: { Text("s") }, sidebarFooter: { Text("f") }, list: { Text("l") }, detail: { Text("d") })
        let host = NSHostingView(rootView: shell)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1000, height: 600),
                              styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.contentView = host
        // SwiftUI creates the chrome probe a beat after the view is installed; poll rather than guess a delay.
        for _ in 0..<40 where window.toolbar == nil {
            host.layoutSubtreeIfNeeded()
            await settle(0.05)
        }
        #expect(window.titleVisibility == .hidden)
        #expect(window.isOpaque == false)
        #expect(window.toolbar != nil)
    }
}
