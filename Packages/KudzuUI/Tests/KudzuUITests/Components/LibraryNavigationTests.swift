import AppKit
@testable import KudzuUI
import SwiftUI
import Testing

@Suite("Library keyboard")
struct LibraryNavigationTests {
    private let id = UUID(uuidString: "00000000-0000-0000-0000-000000000001")!
    private let other = UUID(uuidString: "00000000-0000-0000-0000-000000000002")!

    @Test func escapeClosesTheInnermostSurfaceThenSearchThenSettings() {
        let typing = KzLibraryKeyContext(control: .editorContent, queryIsEmpty: false, transientOpen: true)
        #expect(KzLibraryKeyRouter.route(.escape, in: typing) == .passthrough)

        let trigger = KzLibraryKeyContext(control: .editorTrigger, queryIsEmpty: false, triggerDraftPending: true)
        #expect(KzLibraryKeyRouter.route(.escape, in: trigger) == .cancelTriggerDraft)

        let settings = KzLibraryKeyContext(control: .otherText, inSettings: true, queryIsEmpty: false)
        #expect(KzLibraryKeyRouter.route(.escape, in: settings) == .passthrough)
        #expect(KzLibraryKeyRouter.route(.tab, in: settings) == .passthrough)
        #expect(KzLibraryKeyRouter.route(.shiftTab, in: settings) == .passthrough)

        let searching = KzLibraryKeyContext(control: .search, queryIsEmpty: false)
        #expect(KzLibraryKeyRouter.route(.escape, in: searching) == .clearSearch)

        let editor = KzLibraryKeyContext(control: .editorContent, queryIsEmpty: true)
        #expect(KzLibraryKeyRouter.route(.escape, in: editor) == .focus(.results))

        let results = KzLibraryKeyContext(control: .results, queryIsEmpty: true)
        #expect(KzLibraryKeyRouter.route(.escape, in: results) == .passthrough)

        let rename = KzLibraryKeyContext(control: .rename, queryIsEmpty: false)
        #expect(KzLibraryKeyRouter.route(.escape, in: rename) == .passthrough)
    }

    @Test func tabAndArrowsStayOutOfTextAndCrossPanesInOrder() {
        #expect(KzLibraryKeyRouter.route(.tab, in: KzLibraryKeyContext(control: .search)) == .focus(.results))
        #expect(KzLibraryKeyRouter.route(.shiftTab, in: KzLibraryKeyContext(control: .results)) == .focus(.search))
        #expect(KzLibraryKeyRouter.route(.tab, in: KzLibraryKeyContext(control: .results)) == .focus(.editorName))
        #expect(KzLibraryKeyRouter.route(.tab, in: KzLibraryKeyContext(control: .editorName)) == .focus(.editorTriggers))
        #expect(KzLibraryKeyRouter.route(.shiftTab, in: KzLibraryKeyContext(control: .editorName)) == .focus(.results))
        #expect(KzLibraryKeyRouter.route(.shiftTab, in: KzLibraryKeyContext(control: .editorContent)) == .focus(.editorTriggers))
        #expect(KzLibraryKeyRouter.route(.tab, in: KzLibraryKeyContext(control: .sidebar)) == .focus(.search))
        #expect(KzLibraryKeyRouter.route(.shiftTab, in: KzLibraryKeyContext(control: .search)) == .focus(.sidebar))

        let content = KzLibraryKeyContext(control: .editorContent, hasSingleSelection: true)
        #expect(KzLibraryKeyRouter.route(.down, in: content) == .passthrough)
        #expect(KzLibraryKeyRouter.route(.up, in: content) == .passthrough)
        #expect(KzLibraryKeyRouter.route(.return, in: content) == .passthrough)
        #expect(KzLibraryKeyRouter.route(.delete, in: content) == .passthrough)
        #expect(KzLibraryKeyRouter.route(.tab, in: content) == .passthrough)

        let name = KzLibraryKeyContext(control: .editorName)
        #expect(KzLibraryKeyRouter.route(.delete, in: name) == .passthrough)
        #expect(KzLibraryKeyRouter.route(.down, in: name) == .passthrough)

        #expect(KzLibraryKeyRouter.route(.down, in: KzLibraryKeyContext(control: .search)) == .focus(.results))
        #expect(KzLibraryKeyRouter.route(.return, in: KzLibraryKeyContext(control: .results, hasSingleSelection: true)) == .focus(.editorContent))
        #expect(KzLibraryKeyRouter.route(.return, in: KzLibraryKeyContext(control: .results, hasSingleSelection: false)) == .passthrough)
        #expect(KzLibraryKeyRouter.route(.delete, in: KzLibraryKeyContext(control: .results)) == .deleteSelection)
        #expect(KzLibraryKeyRouter.route(.delete, in: KzLibraryKeyContext(control: .search)) == .passthrough)
        #expect(KzLibraryKeyRouter.route(.down, in: KzLibraryKeyContext(control: .results)) == .passthrough)
    }

    @Test func modifiedKeysAreNotRouted() {
        #expect(KzLibraryKeyParser.key(code: KzLibraryKeyParser.Key.delete, shift: false, command: true, option: false, control: false) == nil)
        #expect(KzLibraryKeyParser.key(code: KzLibraryKeyParser.Key.down, shift: true, command: false, option: false, control: false) == nil)
        #expect(KzLibraryKeyParser.key(code: KzLibraryKeyParser.Key.tab, shift: true, command: false, option: false, control: false) == .shiftTab)
        #expect(KzLibraryKeyParser.key(code: KzLibraryKeyParser.Key.tab, shift: false, command: false, option: false, control: false) == .tab)
        #expect(KzLibraryKeyParser.key(code: KzLibraryKeyParser.Key.escape, shift: false, command: false, option: false, control: false) == .escape)
        #expect(KzLibraryKeyParser.key(code: KzLibraryKeyParser.Key.return, shift: false, command: false, option: false, control: false) == .return)
        #expect(KzLibraryKeyParser.key(code: KzLibraryKeyParser.Key.keypadEnter, shift: false, command: false, option: false, control: false) == .return)
    }

    @Test func focusIdentityPrefersTheTextEditorOverItsContainer() {
        #expect(KzFocusIdentity.control(identifier: "kudzu.search", isTable: false, isChipText: true) == .editorContent)
        #expect(KzFocusIdentity.control(identifier: nil, isTable: true, isChipText: false) == .results)
        #expect(KzFocusIdentity.control(identifier: "kudzu.trigger", isTable: false, isChipText: false) == .editorTrigger)
        #expect(KzFocusIdentity.control(identifier: "kudzu.editor.name", isTable: false, isChipText: false) == .editorName)
        #expect(KzFocusIdentity.control(identifier: "kudzu.rename", isTable: false, isChipText: false) == .rename)
        #expect(KzFocusIdentity.control(identifier: "kudzu.sidebar", isTable: false, isChipText: false) == .sidebar)
        #expect(KzFocusIdentity.control(identifier: "kudzu.settings", isTable: false, isChipText: false) == .otherText)
        #expect(KzFocusIdentity.control(identifier: nil, isTable: false, isChipText: false) == .other)
    }

    @Test func aHiddenSelectionLeavesTheEditorAndEnteringResultsPicksTheFirstRow() {
        let visible = [other]
        #expect(KzSelectionReconciler.reconciled([id], visible: visible).isEmpty)
        #expect(KzSelectionReconciler.reconciled([id, other], visible: visible) == [other])
        #expect(KzSelectionReconciler.reconciled([id], visible: [UUID]()).isEmpty)
        #expect(KzSelectionReconciler.selectionForEnteringResults(current: [id], visible: visible) == [other])
        #expect(KzSelectionReconciler.selectionForEnteringResults(current: [other], visible: [other, id]) == [other])
        #expect(KzSelectionReconciler.selectionForEnteringResults(current: [], visible: []).isEmpty)
    }

    @Test func navigationHoldsASelectionTheEditorWillNotLeave() {
        #expect(KzNavigationGate.decide(current: [id], proposed: [id], allowsLeave: false) == .apply([id]))
        #expect(KzNavigationGate.decide(current: [id], proposed: [], allowsLeave: true) == .apply([]))
        #expect(KzNavigationGate.decide(current: [id], proposed: [], allowsLeave: false) == .hold([]))
        #expect(KzNavigationGate.decide(current: [id], proposed: [other], allowsLeave: false) == .hold([other]))
    }

    @Test func triggerDraftCommitsOnTabAndBlurAndEscapeCancelsOnlyTheDraft() {
        #expect(KzTriggerDraft.handle(.tab, text: ";sig", isComposing: false, isValid: true) == .commitAndMove(forward: true))
        #expect(KzTriggerDraft.handle(.backtab, text: ";sig", isComposing: false, isValid: true) == .commitAndMove(forward: false))
        #expect(KzTriggerDraft.handle(.tab, text: "  ", isComposing: false, isValid: true) == .move(forward: true))
        #expect(KzTriggerDraft.handle(.tab, text: "bad", isComposing: false, isValid: false) == .reject)
        #expect(KzTriggerDraft.handle(.newline, text: "bad", isComposing: false, isValid: false) == .commit)
        #expect(KzTriggerDraft.handle(.escape, text: "bad", isComposing: false, isValid: false) == .cancel)
        #expect(KzTriggerDraft.handle(.escape, text: "", isComposing: false, isValid: true) == .passthrough)
        #expect(KzTriggerDraft.handle(.escape, text: "bad", isComposing: true, isValid: true) == .passthrough)
        #expect(KzTriggerDraft.handle(.tab, text: "に", isComposing: true, isValid: true) == .passthrough)
        #expect(KzTriggerDraft.handle(.newline, text: "に", isComposing: true, isValid: true) == .passthrough)
        #expect(KzTriggerDraft.handle(.deleteBackward, text: "", isComposing: false, isValid: true) == .removeLast)
        #expect(KzTriggerDraft.handle(.deleteBackward, text: ";s", isComposing: false, isValid: true) == .passthrough)

        #expect(KzTriggerDraft.finish(text: ";sig", isComposing: false, cancelled: false, isValid: true) == .commit)
        #expect(KzTriggerDraft.finish(text: "bad", isComposing: false, cancelled: false, isValid: false) == .reject)
        #expect(KzTriggerDraft.finish(text: "", isComposing: false, cancelled: false, isValid: true) == .clearError)
        #expect(KzTriggerDraft.finish(text: ";sig", isComposing: false, cancelled: true, isValid: true) == .keep)
        #expect(KzTriggerDraft.finish(text: "ほん", isComposing: true, cancelled: false, isValid: true) == .keep)
    }
}

@MainActor
@Suite("Main window policy")
struct MainWindowPolicyTests {
    @Test func theMainWindowStaysResizableOnItsOwnSpace() {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 900, height: 640), styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: true)
        window.level = .floating
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient]
        let before = window.frame.size
        KzMainWindowPolicy.apply(to: window)
        #expect(window.level == .normal)
        #expect(window.styleMask.contains(.resizable))
        #expect(!window.styleMask.contains(.fullScreen))
        #expect(window.collectionBehavior.isSuperset(of: KzMainWindowPolicy.required))
        #expect(window.collectionBehavior.isDisjoint(with: KzMainWindowPolicy.disallowed))
        #expect(window.frame.size == before)
        let minimum = KzAppShell<EmptyView, EmptyView, EmptyView, EmptyView, EmptyView>.minimumSize
        #expect(window.contentMinSize.width >= minimum.width)
        #expect(window.contentMinSize.height >= minimum.height)
        #expect(window.contentMaxSize.width > 1040)
        #expect(window.contentMaxSize.height > 680)
    }
}
