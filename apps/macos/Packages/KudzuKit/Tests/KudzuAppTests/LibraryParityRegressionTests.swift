import Foundation
import KudzuCore
import Testing
import SwiftUI
@testable import KudzuApp

@Suite struct LibraryParityRegressionTests {
    private func actions() -> [CommandAction] {
        CommandMenuLogic.actions(selected: nil, paused: false, theme: .dark, clipboardEnabled: true, groups: [])
    }

    @Test func commandQueryRequiresEveryTokenAcrossLabelAndKeywords() {
        let rows = LibraryCommandRows.rows(query: "yaml csv", library: .empty, usage: [:], actions: actions())
        #expect(rows.map(\.id) == ["imp"])
        let reversed = LibraryCommandRows.rows(query: "csv import", library: .empty, usage: [:], actions: actions())
        #expect(reversed.map(\.id) == ["imp"])
        #expect(LibraryCommandRows.rows(query: "nwsnp", library: .empty, usage: [:], actions: actions()).isEmpty)
    }

    @Test func commandSnippetSearchIsNotFilteredAgainAndShowsItsContext() {
        let group = Group(name: "Email")
        let s = Snippet(name: "Signature", triggers: [";sig"], content: "Hello", groupID: group.id)
        let library = Library(groups: [group], snippets: [s])
        let hits = LibraryCommandRows.rows(query: "sig Hello", library: library, usage: [:], actions: [])
        #expect(hits.map(\.snippetID) == [s.id])
        let row = LibraryCommandRows.rows(query: "", library: library, usage: [:], actions: []).first
        #expect(row?.groupPath == "Email")
        #expect(row?.trigger == ";sig")
    }

    @Test func commandsKeepSectionsAndCatalogueOrder() {
        let s = Snippet(name: "Pause meeting")
        let selected = CommandMenuLogic.actions(selected: s, paused: false, theme: .dark, clipboardEnabled: true, groups: [])
        let rows = LibraryCommandRows.rows(query: "pause", library: Library(snippets: [s]), usage: [:], actions: selected)
        #expect(rows.map(\.heading) == ["Snippets", "Selected snippet", "Selected snippet", "Selected snippet", "Selected snippet", "Actions", "Actions"])
        #expect(rows.map(\.id).suffix(2) == ["pause", "clippause"])
    }

    @Test func commandNavigationWrapsAtEitherEnd() {
        let rows = LibraryCommandRows.rows(query: "", library: .empty, usage: [:], actions: actions())
        #expect(LibraryCommandRows.moved(rows.last?.id, by: 1, rows: rows) == rows.first?.id)
        #expect(LibraryCommandRows.moved(rows.first?.id, by: -1, rows: rows) == rows.last?.id)
        #expect(LibraryCommandRows.moved(nil, by: 1, rows: []) == nil)
    }

    @Test func nestedSnippetSummaryResolvesPersistedLowercaseIDs() {
        let id = UUID(uuidString: "abcdefab-abcd-abcd-abcd-abcdefabcdef")!
        let nested = Snippet(id: id, name: "Signature")
        let parent = Snippet(content: "Hello {{snippet id=\"abcdefab-abcd-abcd-abcd-abcdefabcdef\"}}")
        #expect(LibraryRowSummary.text(parent, library: Library(snippets: [parent, nested])) == "Hello ‹↳ Signature›")
        #expect(LibraryRowSummary.text(parent, library: Library(snippets: [parent])) == "Hello ‹↳ Missing snippet›")
    }

    @Test func filterArrowNavigationKeepsTypingInTheFilter() {
        let a = UUID(), b = UUID()
        let start = SnippetListNavigation(selection: ListSelection(current: a, rows: [a], anchor: a), focus: .filter)
        let next = start.moved(in: [a, b], by: 1, extend: true)
        #expect(next.focus == .filter)
        #expect(next.selection.current == b)
        #expect(next.selection.rows == [a, b])
        let fromList = SnippetListNavigation(selection: start.selection, focus: .list).moved(in: [a, b], by: 1, extend: false)
        #expect(fromList.focus == .list)
    }

    @Test func groupRenameAcceptsDoubleClickF2AndOptionReturn() {
        #expect(GroupRenameGesture.startsRename(clickCount: 2))
        #expect(GroupRenameGesture.startsRename(key: "\u{F705}"))
        #expect(GroupRenameGesture.startsRename(key: "\r", option: true))
        #expect(!GroupRenameGesture.startsRename(clickCount: 1))
        #expect(!GroupRenameGesture.startsRename(key: "\r"))
        #expect(!GroupRenameGesture.startsRename(key: "a", option: true))
    }

    @Test func nativeTextEditingOwnsCommandDeleteAndQuestionMark() {
        #expect(!LibraryKeyboardPolicy.canDeleteSelection(inLibrary: true, editingText: true))
        #expect(LibraryKeyboardPolicy.canDeleteSelection(inLibrary: true, editingText: false))
        #expect(!LibraryKeyboardPolicy.canDeleteSelection(inLibrary: false, editingText: false))
        #expect(LibraryKeyboardPolicy.opensShortcuts(character: "?", command: false, control: false, option: false, editingText: false))
        #expect(!LibraryKeyboardPolicy.opensShortcuts(character: "?", command: false, control: false, option: false, editingText: true))
        #expect(!LibraryKeyboardPolicy.opensShortcuts(character: "?", command: true, control: false, option: false, editingText: false))
    }

    @Test func blankSecureInputOwnerUsesTheGenericExplanation() {
        #expect(EngineHealth.secure.detail(owner: "") == "A password field or app has Secure Input on, so macOS blocks expansion.")
    }

    @Test func anyNonemptyCommandQueryAllowsEightSnippetHits() {
        let snippets = (1...8).map { Snippet(name: "Item \($0)") }
        let rows = LibraryCommandRows.rows(query: "  ", library: Library(snippets: snippets), usage: [:], actions: [])
        #expect(rows.count == 8)
        #expect(rows.allSatisfy { $0.heading == "Frequently used" })
    }

    @Test func onlySidebarGroupCreationBeginsInlineRename() {
        #expect(!LibraryGroupCreationSource.windowShortcut.startsRename)
        #expect(!LibraryGroupCreationSource.commandMenu.startsRename)
        #expect(LibraryGroupCreationSource.sidebar.startsRename)
    }

    @Test @MainActor func groupAppPickerSlotReceivesTheLiveRuleBinding() {
        var chosen = ["com.apple.mail"]
        var receivedLabel = ""
        let binding = Binding(get: { chosen }, set: { chosen = $0 })
        let slots = LibrarySlots(groupAppPicker: { value, label in
            receivedLabel = label
            value.wrappedValue.append("com.apple.Notes")
            return AnyView(EmptyView())
        })
        _ = slots.groupAppPicker(binding, "Allow an app")
        #expect(chosen == ["com.apple.mail", "com.apple.Notes"])
        #expect(receivedLabel == "Allow an app")
    }
    @Test func shortcutsContentFitsShortWindowsAndCapsLargeOnes() {
        #expect(LibraryShortcuts.contentHeight(viewportHeight: 560) == 420)
        #expect(LibraryShortcuts.contentHeight(viewportHeight: 760) == 570)
        #expect(LibraryShortcuts.contentHeight(viewportHeight: 1200) == 640)
    }

}
