import Foundation
import KudzuCore
@testable import KudzuEngine
import Testing

/// Public contract tests, including the three tests in `kudzu-engine/tests/engine.rs`.
@Suite struct EngineTests {
    private final class RecordingHost: EngineHost {
        let statuses = Locked<[EngineStatus]>([])
        func statusChanged(_ status: EngineStatus) { statuses.withLock { $0.append(status) } }
        func expanded(_ event: ExpansionEvent) {}
        func formRequested(_ request: FormRequest) {}
    }

    private func library(_ snippets: [Snippet]) -> EngineLibrary {
        EngineLibrary(snippets: snippets, locale: "en_US")
    }

    @Test func rendersNestedSnippetsWithFormDefaults() {
        let engine = Engine(host: RecordingHost())
        let inner = Snippet(name: "inner", content: "Regards")
        let outer = Snippet(name: "outer", triggers: [";sig"], content: "Hi {{field name=who default=there}}, {{choice name=tone options=\"warm|cool\"}}.{{cursor}} {{snippet id=\"\(inner.id.uuidString.lowercased())\"}}")
        engine.reload(library([outer, inner]))
        #expect(engine.renderSnippet(id: outer.id) == "Hi there, warm. Regards")
        #expect(engine.renderSnippet(id: outer.id, formValues: ["who": "Sam"]) == "Hi Sam, warm. Regards")
        #expect(engine.renderSnippet(id: UUID()) == nil)
    }

    @Test func reloadReplacesTheLibrary() {
        let engine = Engine(host: RecordingHost())
        var snippet = Snippet(name: "s", triggers: ["s"], content: "one")
        engine.reload(library([snippet]))
        engine.reload(library([snippet]))
        #expect(engine.renderSnippet(id: snippet.id) == "one")
        snippet.content = "two"
        engine.reload(library([snippet]))
        #expect(engine.renderSnippet(id: snippet.id) == "two")
    }

    @Test func pauseIsReportedOncePerChange() {
        let host = RecordingHost()
        let engine = Engine(host: host)
        engine.setPaused(true)
        engine.setPaused(true)
        #expect(engine.status.paused)
        #expect(engine.status.summary == "Paused")
        engine.setPaused(false)
        #expect(host.statuses.get().map(\.paused) == [true, false])
    }

    @Test func reloadPreservesTheExactUnicodeSource() {
        let engine = Engine(host: RecordingHost())
        var snippet = Snippet(name: "s", triggers: ["s"], content: "\u{e9}")
        engine.reload(library([snippet]))
        // Swift String equality treats these as equal; Rust compares their actual bytes.
        snippet.content = "e\u{301}"
        engine.reload(library([snippet]))
        #expect(engine.renderSnippet(id: snippet.id).map { Array($0.utf16) } == [0x65, 0x301])
    }

    @Test func reloadInvalidatesNormalizedButStaleFormattingSource() {
        let engine = Engine(host: RecordingHost())
        var snippet = Snippet(name: "s", content: "e\u{301}", formatting: Formatting(source: "e\u{301}", spans: [FormatSpan(start: 0, end: 2, style: .bold)]))
        engine.reload(library([snippet]))
        #expect(engine.renderSnippetFormatted(id: snippet.id)?.formatting?.count == 1)
        // An exact source mismatch must disable ranges from another UTF-16 representation.
        snippet.formatting?.source = "\u{e9}"
        engine.reload(library([snippet]))
        #expect(engine.renderSnippetFormatted(id: snippet.id)?.formatting == nil)
    }

    @Test func unchangedReloadPreservesTheLiveBufferAndUndo() throws {
        let runtime = EngineRuntime(host: RecordingHost())
        defer { runtime.shutdown() }
        let snippet = Snippet(name: "s", triggers: [";sig"], content: "Regards")
        let loaded = library([snippet])
        runtime.reload(loaded)
        let match = try #require(runtime.core.withLock { core -> TriggerMatch? in
            guard case .expand(let match) = core.matcher.handle(.character(";sig"), bundleID: nil) else { return nil }
            core.matcher.didExpand(match: match, insertedCount: 7)
            return match
        })
        // A no-op usage save must preserve Backspace undo.
        runtime.reload(loaded)
        #expect(runtime.core.withLock { $0.matcher.handle(.backspace, bundleID: nil) } == .undo(UndoAction(deleteCount: 7, retype: match.typedTrigger)))
        runtime.core.withLock { core in
            core.resetMatcher()
            _ = core.matcher.handle(.character(";si"), bundleID: nil)
        }
        runtime.reload(loaded)
        #expect(runtime.core.withLock { $0.matcher.handle(.character("g"), bundleID: nil) } != nil)
    }

    @Test func canonicallyEquivalentFieldNamesRemainDistinct() {
        let engine = Engine(host: RecordingHost())
        let snippet = Snippet(name: "s", content: "{{field name=\"\u{e9}\" default=A}}/{{field name=\"e\u{301}\" default=B}}")
        engine.reload(library([snippet]))
        #expect(engine.renderSnippet(id: snippet.id) == "A/B")
    }
}
