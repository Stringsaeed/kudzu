import AppKit
import ObjectiveC.runtime
import Testing
@testable import KudzuEditor

// Run this bundle through swiftpm-testing-helper in a foreground .app host with
// KUDZU_CARET_WINDOW_TEST=1. A command-line SwiftPM process cannot become active/key, so the
// default suite skips this window integration test. The probe wraps the *superclass* renderer:
// an actual native blink must pass through KZTextView and deliver a two-point iris rect to it.
@MainActor
@Suite(.serialized) struct CaretWindowTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["KUDZU_CARET_WINDOW_TEST"] == "1", "Requires a foreground AppKit test host; see the test comment."))
    func offscreenWindowUsesTwoPointIrisCaret() throws {
        let app = NSApplication.shared
        let policy = app.activationPolicy()
        let previouslyActive = NSWorkspace.shared.frontmostApplication
        app.setActivationPolicy(.regular)
        app.finishLaunching()
        app.activate(ignoringOtherApps: true)
        defer {
            app.setActivationPolicy(policy)
            previouslyActive?.activate()
        }
        let (editor, _) = makeEditor("caret")
        let view = editor.textView
        let selector = #selector(NSTextView.drawInsertionPoint(in:color:turnedOn:))
        let method = try #require(class_getInstanceMethod(NSTextView.self, selector))
        let original = method_getImplementation(method)
        typealias Draw = @convention(c) (NSTextView, Selector, NSRect, NSColor, Bool) -> Void
        let draw = unsafeBitCast(original, to: Draw.self)
        var drawnRects: [NSRect] = []
        var drawnColors: [NSColor] = []
        let probe: @convention(block) (NSTextView, NSRect, NSColor, Bool) -> Void = { target, rect, color, on in
            if target === view, on { drawnRects.append(rect); drawnColors.append(color) }
            draw(target, selector, rect, color, on)
        }
        let replacement = imp_implementationWithBlock(probe)
        method_setImplementation(method, replacement)
        defer {
            method_setImplementation(method, original)
            imp_removeBlock(replacement)
        }
        let window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: 480, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        window.contentView = editor.scrollView
        window.makeKeyAndOrderFront(nil)
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.2))
        window.makeKey()
        #expect(window.makeFirstResponder(view))
        view.updateInsertionPointStateAndRestartTimer(true)
        window.displayIfNeeded()
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.15))
        #expect(app.isActive && window.isKeyWindow)
        #expect(view.shouldDrawInsertionPoint)
        #expect(!drawnRects.isEmpty)
        #expect(drawnRects.allSatisfy { $0.width == 2 })
        #expect(drawnColors.allSatisfy { $0 == view.insertionPointColor })
    }
}
