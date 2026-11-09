import Foundation
import KudzuCore
@testable import KudzuEngine
import Testing

// Port of the tests in `kudzu-engine/src/library.rs`.
@Suite struct LibraryTests {
    private func library(_ snippets: [Snippet], _ groups: [Group]) -> LibrarySnapshot {
        LibrarySnapshot(EngineLibrary(snippets: snippets, groups: groups, config: MatchingConfig(), locale: ""), fallbackLocale: { "en_GB" })
    }

    private func snippet(_ name: String, _ content: String) -> Snippet {
        Snippet(name: name, triggers: [name], content: content)
    }

    @Test func emptyLocaleFallsBackAndDuplicatesKeepFirst() {
        let a = snippet("a", "first")
        var b = a
        b.content = "second"
        let lib = library([a, b], [])
        #expect(lib.locale == "en_GB")
        #expect(lib.snippets.count == 1)
        #expect(lib.source(a.id) == "first")
    }

    @Test func insertionPrecedence() {
        var parent = Group(name: "parent")
        parent.insertion = .type
        var child = Group(name: "child")
        child.parentID = parent.id
        var s = snippet("s", "x")
        s.groupID = child.id
        let lib = library([s], [parent, child])

        // Inherited from the grandparent group, even in a terminal.
        #expect(lib.insertion(for: s, bundleID: "com.apple.Terminal") == .type)

        // Snippet override beats the group.
        var own = s
        own.insertion = .paste
        #expect(lib.insertion(for: own, bundleID: nil) == .paste)

        // A rule's per-app override wins for its listed apps only.
        own.rule = AppRule(mode: .allow, bundleIDs: ["com.example.app"], insertion: .type)
        #expect(lib.insertion(for: own, bundleID: "com.example.app") == .type)
        #expect(lib.insertion(for: own, bundleID: "com.other") == .paste)
    }

    @Test func insertionFallsBackToGlobalSettings() {
        let s = snippet("s", "x")
        let lib = library([s], [])
        #expect(lib.insertion(for: s, bundleID: "com.apple.Terminal") == .paste)
        #expect(lib.insertion(for: s, bundleID: "com.apple.TextEdit") == .automatic)
        #expect(lib.insertion(for: s, bundleID: nil) == .automatic)
    }

    @Test func groupCyclesTerminate() {
        var a = Group(name: "a")
        var b = Group(name: "b")
        a.parentID = b.id
        b.parentID = a.id
        var s = snippet("s", "x")
        s.groupID = a.id
        let lib = library([s], [a, b])
        #expect(lib.insertion(for: s, bundleID: nil) == .automatic)
    }

    @Test func deniedApps() {
        let lib = library([], [])
        #expect(lib.isDenied("com.apple.loginwindow"))
        #expect(!lib.isDenied("com.apple.TextEdit"))
        #expect(!lib.isDenied(nil))
    }

    @Test func defaultsFillMissingValuesOnly() {
        let fields = [
            FormFieldSpec(name: "a", kind: .text, options: [], defaultValue: "A"),
            FormFieldSpec(name: "b", kind: .choice, options: ["x", "y"], defaultValue: nil),
            FormFieldSpec(name: "c", kind: .text, options: [], defaultValue: nil),
        ]
        let values = LibrarySnapshot.withDefaults(fields, ["a": "given"])
        #expect(values["a"] == "given")
        #expect(values["b"] == "x")
        #expect(values["c"] == "")
    }

    // Beyond the Rust suite: the engine-side helpers the worker relies on.

    @Test func clipboardUseIsDetectedThroughNestedSnippets() {
        let inner = snippet("inner", "{{clipboard}}")
        let outer = snippet("outer", "Pasted: {{snippet id=\(inner.id.uuidString.lowercased())}}")
        let plain = snippet("plain", "no tags")
        let lib = library([inner, outer, plain], [])
        #expect(lib.usesClipboard(inner))
        #expect(lib.usesClipboard(outer))
        #expect(!lib.usesClipboard(plain))
    }

    @Test func renderUsesClipboardValuesAndCursor() {
        let s = snippet("s", "Hi {{field name=who}}, {{cursor}}{{clipboard}}")
        let lib = library([s], [])
        let fields = lib.formFields(s)
        #expect(fields.map(\.name) == ["who"])
        let result = lib.render(s, clipboard: "copied", formValues: LibrarySnapshot.withDefaults(fields, ["who": "Ada"]))
        #expect(result.text == "Hi Ada, copied")
        #expect(result.cursorOffset == 8)
    }

    @Test func dateOffsetsKeepTheRenderInstantsUTCOffset() throws {
        let zone = try #require(TimeZone(identifier: "America/New_York"))
        // Noon EST, the day before the spring DST change. Rust passes DateTime<FixedOffset>.
        let now = Date(timeIntervalSince1970: 1_772_902_800)
        let s = snippet("s", "{{date format=\"yyyy-MM-dd HH:mm XXX\" offset=\"+1d\"}}")
        let lib = library([s], [])
        let result = lib.render(s, now: now, timeZone: zone, clipboard: nil, formValues: [:])
        #expect(result.text == "2026-03-08 12:00 -05:00")
    }
}
