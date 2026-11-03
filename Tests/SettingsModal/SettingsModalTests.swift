import AppKit
import SwiftUI
import Testing
@testable import SettingsModalSupport

@Test(arguments: [CGSize(width: 100, height: 80), CGSize(width: 560, height: 400),
                  CGSize(width: 880, height: 480), CGSize(width: 1800, height: 1200), .zero])
func modalFitsAvailableContent(size: CGSize) {
    let frame = SettingsModalLayout.frame(in: size)
    #expect(frame.minX >= 0)
    #expect(frame.minY >= 0)
    #expect(frame.maxX <= size.width)
    #expect(frame.maxY <= size.height)
    #expect(frame.width <= 960)
    #expect(frame.height <= 760)
    #expect(frame.midX == size.width / 2)
    #expect(frame.midY == size.height / 2)
}

@Test @MainActor func nestedPresentationsKeepOuterModalOpen() {
    let window = NSWindow(contentRect: .zero, styleMask: [.titled], backing: .buffered, defer: false)
    let child = NSPanel(contentRect: .zero, styleMask: [.borderless], backing: .buffered, defer: false)
    // Test the routing policy without ordering a window on screen or generating input.
    #expect(SettingsModalFocusSession.canHandleKeys(in: window, keyWindow: window, hasNestedPresentation: false))
    #expect(!SettingsModalFocusSession.canHandleKeys(in: window, keyWindow: child, hasNestedPresentation: false))
    #expect(!SettingsModalFocusSession.canHandleKeys(in: window, keyWindow: window, hasNestedPresentation: true))
    #expect(!SettingsModalFocusSession.canHandleKeys(in: window, keyWindow: nil, hasNestedPresentation: false))
    #expect(!SettingsModalFocusSession.canHandleKeys(in: window, keyWindow: window,
                hasNestedPresentation: false, keyboardCaptured: true))
}

@Test @MainActor func tabTraversalSkipsBackgroundAndWraps() {
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 600),
                          styleMask: [.titled], backing: .buffered, defer: false)
    let background = NSTextField()
    let modal = NSView()
    let first = NSTextField()
    let last = NSTextField()
    window.contentView?.addSubview(background)
    window.contentView?.addSubview(modal)
    modal.addSubview(first)
    modal.addSubview(last)
    first.nextKeyView = last
    last.nextKeyView = background
    background.nextKeyView = first
    #expect(SettingsModalFocusSession.nextView(from: last, in: modal, backwards: false) === first)
    #expect(SettingsModalFocusSession.nextView(from: first, in: modal, backwards: true) === last)
    #expect(SettingsModalFocusSession.nextView(from: first, in: modal, backwards: false) === last)
}

private final class FocusTestWindow: NSWindow {
    override var isKeyWindow: Bool { true }
}

private final class FocusTestHost: NSView {
    override var acceptsFirstResponder: Bool { true }
}

private final class FullscreenTestWindow: NSWindow {
    var simulatedChildren: [NSWindow] = []
    var simulatedSheet: NSWindow?
    override var childWindows: [NSWindow]? { simulatedChildren }
    override var attachedSheet: NSWindow? { simulatedSheet }
    override var isKeyWindow: Bool { true }
}

private final class VisibleFullscreenChrome: NSWindow {
    override var isVisible: Bool { true }
    override var canBecomeKey: Bool { false }
}

private final class VisibleDialog: NSPanel {
    override var isVisible: Bool { true }
    override var canBecomeKey: Bool { true }
}

@Test @MainActor func fullscreenChromeDoesNotBlockTheCloseButton() {
    let window = FullscreenTestWindow(contentRect: .zero, styleMask: [.titled], backing: .buffered, defer: false)
    let chrome = VisibleFullscreenChrome(contentRect: .zero, styleMask: [.borderless], backing: .buffered, defer: false)
    chrome.collectionBehavior = [.fullScreenAuxiliary]
    window.simulatedChildren = [chrome]
    let host = FocusTestHost()
    window.contentView?.addSubview(host)
    let session = SettingsModalFocusSession()
    var closed = false
    session.close = { closed = true }
    session.begin(in: host)
    defer { session.end() }
    #expect(!SettingsModalFocusSession.hasNestedPresentation(in: window))
    session.requestClose()
    #expect(closed)
}

@Test @MainActor func explicitCloseDoesNotDependOnKeyboardHostAttachment() {
    let session = SettingsModalFocusSession()
    var closed = false
    session.close = { closed = true }
    session.requestClose()
    #expect(closed)
}

@Test @MainActor func theCloseButtonStillRespectsAnActualDialog() {
    let window = FullscreenTestWindow(contentRect: .zero, styleMask: [.titled], backing: .buffered, defer: false)
    window.simulatedChildren = [VisibleDialog(contentRect: .zero, styleMask: [.borderless], backing: .buffered, defer: false)]
    window.simulatedSheet = window.simulatedChildren.first
    let host = FocusTestHost()
    window.contentView?.addSubview(host)
    let session = SettingsModalFocusSession()
    var closed = false
    session.close = { closed = true }
    session.begin(in: host)
    defer { session.end() }
    #expect(SettingsModalFocusSession.hasNestedPresentation(in: window))
    session.requestClose()
    #expect(!closed)
}

@Test @MainActor func closingRestoresEditorAndInsertionSelection() {
    let window = FocusTestWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 600),
                                 styleMask: [.titled], backing: .buffered, defer: false)
    let editor = NSTextView()
    editor.string = "An unsaved snippet"
    let host = FocusTestHost()
    window.contentView?.addSubview(editor)
    window.contentView?.addSubview(host)
    #expect(window.makeFirstResponder(editor))
    editor.setSelectedRange(NSRange(location: 3, length: 7))
    let session = SettingsModalFocusSession()
    session.begin(in: host)
    #expect(window.firstResponder === host)
    // SwiftUI may detach the host before calling dismantleNSView.
    host.removeFromSuperview()
    session.end()
    #expect(window.firstResponder === editor)
    #expect(editor.selectedRange() == NSRange(location: 3, length: 7))
    #expect(editor.string == "An unsaved snippet")
}

@Test @MainActor func closingDoesNotRestoreADetachedEditor() {
    let window = FocusTestWindow(contentRect: .zero, styleMask: [.titled], backing: .buffered, defer: false)
    let editor = NSTextView()
    let host = FocusTestHost()
    window.contentView?.addSubview(editor)
    window.contentView?.addSubview(host)
    window.makeFirstResponder(editor)
    let session = SettingsModalFocusSession()
    session.begin(in: host)
    editor.removeFromSuperview()
    session.end()
    #expect(window.firstResponder !== editor)
}

@Test @MainActor func hostingSubtreeExposesKeyboardTargets() {
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 600),
                          styleMask: [.titled], backing: .buffered, defer: false)
    let host = NSHostingView(rootView: VStack {
        TextField("Settings value", text: .constant("Example"))
        Button("Close settings") {}
    })
    host.frame = NSRect(x: 20, y: 20, width: 600, height: 400)
    window.contentView?.addSubview(host)
    host.layoutSubtreeIfNeeded()
    window.recalculateKeyViewLoop()
    let next = SettingsModalFocusSession.nextView(from: host, in: host, backwards: false)
    #expect(next != nil)
    #expect(next !== host)
}

@Test @MainActor func libraryFindIsBlockedButNativeFullscreenShortcutIsNot() {
    #expect(SettingsModalFocusSession.blocksLibraryShortcut(characters: "f", modifiers: .command))
    #expect(!SettingsModalFocusSession.blocksLibraryShortcut(characters: "f", modifiers: [.command, .control]))
    #expect(!SettingsModalFocusSession.blocksLibraryShortcut(characters: "c", modifiers: .command))
    #expect(!SettingsModalFocusSession.blocksLibraryShortcut(characters: "v", modifiers: .command))
    #expect(!SettingsModalFocusSession.blocksLibraryShortcut(characters: "q", modifiers: .command))
}
