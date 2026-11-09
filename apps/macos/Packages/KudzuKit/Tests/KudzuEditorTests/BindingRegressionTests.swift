import AppKit
import KudzuCore
import SwiftUI
import Testing
@testable import KudzuEditor

@MainActor
@Suite(.serialized) struct BindingRegressionTests {
    @Test func canonicallyEquivalentEditUpdatesFormattingSourceExactly() throws {
        var value = "a\u{301}\u{323}"
        var formatting: Formatting? = Formatting(source: value, spans: [])
        let handle = TemplateEditorHandle()
        let view = TemplateTextView(value: Binding(get: { value }, set: { value = $0 }), formatting: Binding(get: { formatting }, set: { formatting = $0 }), handle: handle)
        let host = NSHostingView(rootView: view)
        host.frame = NSRect(x: 0, y: 0, width: 480, height: 240)
        host.layoutSubtreeIfNeeded()
        let controller = try #require(handle.controller)
        controller.replace(0..<3, with: "a\u{323}\u{301}")
        #expect(utf16Equal(value, "a\u{323}\u{301}"))
        #expect(formatting.map { utf16Equal($0.source, value) } == true)
        #expect(FormattingHelpers.sourceSpans(value, formatting: formatting) == [])
        withExtendedLifetime(host) {}
    }
    @Test func linkPopoverSubmissionComparesExactURLText() {
        let (editor, _) = makeEditor("link", spans: [FormatSpan(start: 0, end: 4, style: .link(url: "https://a/é"))], selection: 0..<4)
        let handle = TemplateEditorHandle()
        handle.attach(editor)
        editor.onStateChange = { handle.sync() }
        handle.applyLink("https://a/e\u{301}")
        #expect(editor.state.linkAtSelection.map { utf16Equal($0, "https://a/e\u{301}") } == true)
        let changes = editor.undoManager.canUndo
        handle.applyLink("https://a/e\u{301}")
        #expect(editor.state.spans?.count == 1)
        #expect(changes)
    }

}
