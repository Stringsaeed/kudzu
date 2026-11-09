import AppKit
import KudzuCore
import SwiftUI
import Testing
@testable import KudzuEditor

/// The snippet editor grows with its content from a minimum (`min-h-[168px]`) and leaves scrolling
/// to the pane around it.
@MainActor
@Suite(.serialized) struct GrowingEditorTests {
    @Test func trailingEmptyLineKeepsItsLineBoxAndBottomInset() throws {
        let (editor, _) = makeEditor(String(repeating: "line\n", count: 20))
        editor.growsFrom = 168
        let caret = try #require(editor.caretRect(atStorage: editor.textView.string.utf16.count))
        #expect(editor.fittingHeight(width: 480) >= caret.maxY + TemplateTextController.inset.height)
        #expect(editor.fittingHeight(width: 240) >= caret.maxY + TemplateTextController.inset.height)
        editor.replace(0..<editor.state.source.utf16.count, with: "")
        #expect(editor.fittingHeight(width: 480) == 168)
    }

    @Test func shortContentKeepsTheMinimumAndLongContentGrows() throws {
        var value = "Hello"
        let handle = TemplateEditorHandle()
        let view = TemplateTextView(value: Binding(get: { value }, set: { value = $0 }), handle: handle, minHeight: 168)
            .frame(width: 480).fixedSize(horizontal: false, vertical: true)
        let host = NSHostingView(rootView: view)
        host.frame = NSRect(x: 0, y: 0, width: 480, height: 900)
        host.layoutSubtreeIfNeeded()
        let controller = try #require(handle.controller)
        #expect(abs(controller.scrollView.frame.height - 168) < 0.5)
        #expect(controller.scrollView.hasVerticalScroller == false)

        controller.replace(5..<5, with: String(repeating: "\nline", count: 20))
        host.layoutSubtreeIfNeeded()
        let grown = controller.scrollView.frame.height
        // 21 lines at 13 pt × 1.7, plus the 12 pt top and bottom insets.
        #expect(abs(grown - ceil(21 * 13 * 1.7 + 24)) < 1)
        #expect(controller.textView.frame.height <= grown)

        controller.replace(0..<value.utf16.count, with: "Short again")
        host.layoutSubtreeIfNeeded()
        #expect(abs(controller.scrollView.frame.height - 168) < 0.5)
        withExtendedLifetime(host) {}
    }

    @Test func narrowerWidthsWrapIntoMoreLines() {
        let (editor, _) = makeEditor(String(repeating: "word ", count: 60))
        editor.growsFrom = 0
        let wide = editor.fittingHeight(width: 480)
        let narrow = editor.fittingHeight(width: 240)
        #expect(narrow > wide)
        // Measuring at another width leaves the container matching the view.
        #expect(abs((editor.textView.textContainer?.size.width ?? 0) - (480 - 28)) < 0.5)
    }

    @Test func withoutAMinimumTheEditorScrollsInside() throws {
        var value = "Hello"
        let handle = TemplateEditorHandle()
        let host = NSHostingView(rootView: TemplateTextView(value: Binding(get: { value }, set: { value = $0 }), handle: handle).frame(width: 480, height: 240))
        host.frame = NSRect(x: 0, y: 0, width: 480, height: 240)
        host.layoutSubtreeIfNeeded()
        let controller = try #require(handle.controller)
        #expect(controller.scrollView.frame.height == 240)
        #expect(controller.scrollView.hasVerticalScroller)
        withExtendedLifetime(host) {}
    }

    @Test func wheelScrollingTargetsThePaneAroundTheEditor() {
        let (editor, _) = makeEditor("Hello")
        editor.growsFrom = 168
        let outer = NSScrollView(frame: NSRect(x: 0, y: 0, width: 300, height: 120))
        let document = NSView(frame: NSRect(x: 0, y: 0, width: 300, height: 800))
        outer.documentView = document
        document.addSubview(editor.scrollView)
        let scroll = editor.scrollView as? EditorScrollView
        #expect(scroll?.forwardsScrolling == true)
        #expect(scroll?.hasVerticalScroller == false)
        #expect(scroll?.nearestAncestorScrollView() === outer)
        #expect(editor.fittingHeight(width: 300) >= 168)
    }
}
