import Foundation
import KudzuCore
import Testing
@testable import KudzuApp

/// Palette behaviour ported from `palette.tsx`: the `#group` query, scope cycling, keyboard
/// movement, the create row, and row glyphs. Rendering stays out of these tests.
@MainActor @Suite struct PaletteTests {
    private func snippet(_ name: String, _ triggers: [String], content: String = "Hello", favorite: Bool = false, group: UUID? = nil, enabled: Bool = true) -> Snippet {
        Snippet(name: name, triggers: triggers, content: content, groupID: group, isFavorite: favorite, isEnabled: enabled)
    }

    private func palette(_ library: Library, query: String = "") -> PaletteModel {
        let model = PaletteModel(backend: MockBackend(), library: library)
        if !query.isEmpty { model.query = query }
        return model
    }

    // MARK: Query

    @Test func parseSplitsTheFirstGroupTerm() {
        #expect(PaletteModel.parse("invoice") == ("invoice", nil))
        #expect(PaletteModel.parse("#work invoice") == ("invoice", "work"))
        #expect(PaletteModel.parse("invoice #work extra") == ("invoice  extra", "work"))
        #expect(PaletteModel.parse("#") == ("", ""))
        #expect(PaletteModel.parse("# work") == ("work", ""))
        #expect(PaletteModel.parse("hello#") == ("hello#", nil))
    }

    @Test func groupPrefixKeepsSnippetsWhoseGroupNameStartsWithTheTerm() {
        let email = Group(name: "Email")
        let dev = Group(name: "Developer")
        let library = Library(groups: [email, dev], snippets: [
            snippet("Signature", [";sig"], group: email.id),
            snippet("Bug", [";bug"], group: dev.id),
            snippet("Loose", [";loose"]),
        ])
        let model = palette(library, query: "#em sig")
        #expect(model.groupFilter == "em")
        #expect(model.text == "sig")
        #expect(model.hits.map(\.snippet.name) == ["Signature"])
        #expect(model.path(of: model.hits[0].snippet) == "Email")
    }

    @Test func hashAloneHidesUngroupedSnippets() {
        let work = Group(name: "Work")
        let library = Library(groups: [work], snippets: [
            snippet("Calendar", [";cal"], group: work.id),
            snippet("Thanks", [";ty"]),
        ])
        let model = palette(library, query: "#")
        #expect(model.hits.map(\.snippet.name) == ["Calendar"])
        #expect(model.groupFilter == "")
        #expect(model.sectionLabel == "Suggested")
    }

    // MARK: Rows

    @Test func emptyQueryPutsFavoritesFirst() {
        let library = Library(snippets: [
            snippet("Alpha", [";a"]),
            snippet("Starred", [";s"], favorite: true),
            snippet("Beta", [";b"]),
        ])
        let model = palette(library)
        #expect(model.hits.first?.snippet.name == "Starred")
        #expect(model.sectionLabel == "Suggested")
        #expect(model.createRow == false)
    }

    @Test func recentScopeSortsByLastUseAndDropsUnused() {
        let used = snippet("Old", [";old"])
        let newer = snippet("New", [";new"])
        let never = snippet("Never used", [";no"])
        let library = Library(snippets: [used, newer, never], usage: [
            UsageRecord(snippetID: used.id, count: 2, lastUsedAt: Date(timeIntervalSince1970: 1_000)),
            UsageRecord(snippetID: newer.id, count: 1, lastUsedAt: Date(timeIntervalSince1970: 5_000)),
        ])
        let model = palette(library)
        model.setScope(.recent)
        #expect(model.hits.map(\.snippet.name) == ["New", "Old"])
        #expect(model.sectionLabel == "Recently used")
        #expect(model.emptyMessage == "Nothing used yet.")
    }

    @Test func createRowAppearsUnlessTheQueryIsExactlyATrigger() {
        let library = Library(snippets: [snippet("Signature", [";sig", ";signature"])])
        let typed = palette(library, query: "Sig")
        #expect(typed.createRow)
        #expect(typed.total == typed.hits.count + 1)
        let exact = palette(library, query: ";SIG")
        #expect(exact.createRow == false)
    }

    @Test func highlightsFollowUTF16Indexes() {
        #expect(PaletteHighlight.runs("Signature", hits: [0, 1, 2]).map(\.text) == ["Sig", "nature"])
        #expect(PaletteHighlight.runs("Signature", hits: [0, 1, 2]).map(\.hit) == [true, false])
        #expect(PaletteHighlight.runs("Café", hits: []).map(\.hit) == [false])
        #expect(PaletteHighlight.runs("", hits: [0]).isEmpty)
    }

    @Test func kindPrefersFormThenClipboardThenDate() {
        #expect(PaletteKind.classify("Hi {{field name=\"Name\"}}") == .form)
        #expect(PaletteKind.classify("> {{clipboard trim=true}}") == .clipboard)
        #expect(PaletteKind.classify("{{date}}") == .date)
        #expect(PaletteKind.classify("{{time format=\"h:mm a\"}}") == .date)
        #expect(PaletteKind.classify("Thanks") == .plain)
        #expect(PaletteKind.classify("{{field name=\"A\"}} {{clipboard}}") == .form)
    }

    @Test func usedLabelSaysNeverOrTheCount() {
        let snippet = snippet("Today", [";date"])
        let library = Library(snippets: [snippet], usage: [
            UsageRecord(snippetID: snippet.id, count: 3, lastUsedAt: Date(timeIntervalSince1970: 10_000)),
        ])
        let model = palette(library)
        let now = Date(timeIntervalSince1970: 10_000)
        #expect(model.usedLabel(snippet, now: now) == "3× · just now")
        #expect(model.usedLabel(self.snippet("Other", [";x"]), now: now) == "Never")
    }

    // MARK: Keys

    @Test func arrowsWrapAndPageStopsAtTheEnds() {
        let library = Library(snippets: (0..<3).map { snippet("S\($0)", [";\($0)"]) })
        let model = palette(library)
        #expect(model.total == 3)
        model.press(.up)
        #expect(model.clamped == 2)
        model.press(.down)
        #expect(model.clamped == 0)
        model.press(.pageDown)
        #expect(model.clamped == 2)
        model.press(.pageDown)
        #expect(model.clamped == 2)
        model.press(.pageUp)
        #expect(model.clamped == 0)
    }

    @Test func tabCyclesScopesAndEscapeClearsThenHides() {
        let model = palette(Library(snippets: [snippet("A", [";a"])]), query: "a")
        #expect(model.press(.tab(shift: false)) == nil)
        #expect(model.scope == .favorites)
        model.press(.tab(shift: true))
        #expect(model.scope == .all)
        #expect(model.press(.escape) == nil)
        #expect(model.query.isEmpty)
        #expect(model.press(.escape) == .hide)
    }

    @Test func returnCopiesEditsNumbersAndCreates() {
        let model = palette(Library(snippets: [snippet("Alpha", [";a"]), snippet("Beta", [";b"])]))
        let first = model.hits[0].snippet.id
        let second = model.hits[1].snippet.id
        #expect(model.press(.enter(command: false)) == .insert(first))
        #expect(model.press(.enter(command: true)) == .copy(first))
        #expect(model.flash == "Copied")
        #expect(model.press(.edit) == .edit(first))
        #expect(model.press(.number(2)) == .insert(second))
        #expect(model.press(.number(9)) == nil)
        model.query = "note"
        #expect(model.press(.new) == .create("note"))
        model.active = model.hits.count
        #expect(model.press(.enter(command: false)) == .create("note"))
        #expect(model.press(.edit) == nil)
    }

    @Test func chordsMatchTheWebOrder() {
        #expect(PaletteKeys.interpret(.down, command: false, control: false, shift: false) == .down)
        #expect(PaletteKeys.interpret(.character("n"), command: false, control: true, shift: false) == .down)
        #expect(PaletteKeys.interpret(.character("p"), command: false, control: true, shift: false) == .up)
        #expect(PaletteKeys.interpret(.character("n"), command: true, control: true, shift: false) == .down)
        #expect(PaletteKeys.interpret(.character("n"), command: true, control: false, shift: false) == .new)
        #expect(PaletteKeys.interpret(.character("e"), command: true, control: false, shift: false) == .edit)
        #expect(PaletteKeys.interpret(.character("4"), command: true, control: false, shift: false) == .number(4))
        #expect(PaletteKeys.interpret(.return, command: true, control: false, shift: false) == .enter(command: true))
        #expect(PaletteKeys.interpret(.tab, command: false, control: false, shift: true) == .tab(shift: true))
        #expect(PaletteKeys.interpret(.escape, command: false, control: false, shift: false) == .escape)
        #expect(PaletteKeys.interpret(.character("a"), command: false, control: false, shift: false) == nil)
    }

    @Test func footerNamesTheTargetApp() {
        let model = palette(Library(snippets: [snippet("A", [";a"])]))
        #expect(model.footerLabel == "Insert")
        model.show(target: AppInfo(bundleId: "com.apple.mail", name: "Mail"))
        #expect(model.footerLabel == "Insert into Mail")
        model.press(.enter(command: true))
        #expect(model.footerLabel == "Copied")
    }
}
