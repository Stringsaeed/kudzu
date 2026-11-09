import Foundation
import KudzuCore
import Testing
@testable import KudzuApp

@Suite("Editor behavior port")
struct EditorLogicTests {
    @Test func nearestAncestorAndCycles() {
        let parent = SnippetGroup(name: "Parent", insertion: .paste)
        var child = SnippetGroup(name: "Child", parentID: parent.id)
        #expect(EditorLogic.inherited([parent, child], groupID: child.id, key: \.insertion)?.from.id == parent.id)
        child.insertion = .type
        #expect(EditorLogic.inherited([parent, child], groupID: child.id, key: \.insertion)?.value == .type)
        child.insertion = nil
        child.parentID = child.id
        #expect(EditorLogic.inherited([child], groupID: child.id, key: \.insertion) == nil)
    }

    @Test func emptyAppRulesHaveDifferentMeaning() {
        #expect(EditorLogic.describeRule(AppRule(mode: .allow), apps: []) == "Disabled everywhere")
        #expect(EditorLogic.describeRule(AppRule(mode: .deny), apps: []) == "Works in every app")
        #expect(EditorLogic.describeRule(AppRule(mode: .allow, bundleIDs: ["a", "b", "c", "d"]), apps: []) == "Only in a, b, c +1")
    }

    @Test func triggerPasteIsAtomicAndBackspaceEditsLast() {
        var invisible = TriggerEntry(draft: "\u{200B}")
        #expect(invisible.canCommit(prefix: ";"))
        #expect(invisible.commit(triggers: [], caseSensitive: false) == ["\u{200B}"])
        var entry = TriggerEntry(draft: ";one ;ONE")
        #expect(entry.commit(triggers: [], caseSensitive: false) == nil)
        #expect(entry.error == "This snippet already has that trigger.")
        #expect(entry.commit(";one ;two", triggers: [], caseSensitive: false) == [";one", ";two"])
        #expect(entry.backspace(triggers: [";one", ";two"]) == [";one"])
        #expect(entry.draft == ";two")
    }

    @Test func untouchedPrefixDoesNotBecomeTrigger() {
        var entry = TriggerEntry()
        entry.focused(triggers: [], prefix: ";")
        #expect(!entry.canCommit(prefix: ";"))
        #expect(entry.blurred(triggers: [], prefix: ";", caseSensitive: false) == nil)
        #expect(entry.draft.isEmpty)
    }

    @Test func choiceEditsEscapePipesAndPreserveUnknownAttributes() throws {
        let source = "{{choice name=Option options=\"Yes|No\" future=true}}"
        let tag = try #require(Template.tokenize(source).compactMap { if case .tag(let t) = $0 { t } else { nil } }.first)
        var draft = EditorTagDraft(tag)
        draft.set("options", Template.joinOptions(["A|B", "C"]))
        draft.set("default", "A|B")
        let result = Template.render(draft.source, context: RenderContext(now: Date()))
        #expect(result.text == "A|B")
        #expect(draft.source.contains("future=true"))
    }

    @Test func shadowingWarningsPointInBothDirections() {
        let a = UUID(), b = UUID()
        let c = TriggerConflict(kind: .shadowing, snippetID: a, trigger: ";a", otherSnippetID: b, otherTrigger: ";abc")
        #expect(EditorLogic.conflictMessage(c, snippetID: a) == "“;a” fires before “;abc” can be finished in")
        #expect(EditorLogic.conflictMessage(c, snippetID: b) == "“;abc” can't be typed in full — “;a” fires first in")
    }
}
