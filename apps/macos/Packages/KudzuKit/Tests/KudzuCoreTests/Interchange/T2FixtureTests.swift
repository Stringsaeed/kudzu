import Foundation
import Testing
@testable import KudzuCore

func t2Fixture<T: Decodable>(_ type: T.Type, _ path: String) throws -> T {
    let root = URL(fileURLWithPath:#filePath).deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("Fixtures")
    return try KudzuJSON.decode(type,from:Data(contentsOf:root.appendingPathComponent(path)))
}

@Suite struct T2FixtureTests {
    struct Finding: Decodable { var start: Int; var end: Int; var kind: SecretKind }
    struct SecretCase: Decodable { var text: String; var findings: [Finding]; var detect: SecretKind?; var redacted: String; var kinds: [SecretKind]; var whole: Bool }
    struct Corpus: Decodable { var name: String; var cases: [SecretCase] }
    @Test func secretsFixtureReplay() throws {
        for corpus in try t2Fixture([Corpus].self,"secrets/corpus.json") {
            for c in corpus.cases {
                let scanned = Secrets.scan(c.text), r = Secrets.redact(c.text)
                #expect(scanned.count == c.findings.count, "\(corpus.name): \(c.text)")
                for (a,b) in zip(scanned,c.findings) { #expect(a.range == b.start..<b.end); #expect(a.kind == b.kind) }
                #expect(Secrets.detect(c.text) == c.detect); #expect(r.text == c.redacted, "\(corpus.name): \(c.text)"); #expect(r.kinds == c.kinds); #expect(r.whole == c.whole)
            }
        }
    }
    struct Classification: Decodable {
        struct Expected: Decodable { var kind: ClipKind; var text: String?; var files: [String]; var color: String?; var rich: Bool; var imageType: String?; var types: [String]; var size: UInt64; var hash: String }
        var text: String; var source: String?; var expected: Expected?; var blob: [UInt8]
    }
    @Test func clipboardClassificationFixtureReplay() throws {
        for c in try t2Fixture([Classification].self,"clipboard/classification.json") {
            let items: [Clipboard.PasteItem] = [[(Clipboard.uti.PLAIN_TEXT,Data(c.text.utf8))]], result = Clipboard.classify(items,sourceBundle:c.source)
            #expect(Clipboard.encodeItems(items) == Data(c.blob))
            #expect((result == nil) == (c.expected == nil))
            if let a = result, let b = c.expected { #expect(a.kind == b.kind); #expect(a.text == b.text); #expect(a.files == b.files); #expect(a.color == b.color); #expect(a.rich == b.rich); #expect(a.imageType == b.imageType); #expect(a.types == b.types); #expect(a.size == b.size); #expect(String(format:"%016llx",a.hash) == b.hash) }
        }
    }
    struct HistoryFixture: Decodable { var texts: [String]; var states: [Clipboard.History] }
    @Test func clipboardHistoryFixtureReplay() throws {
        let fixture = try t2Fixture(HistoryFixture.self,"clipboard/history.json"); var history = Clipboard.History(), ids: [UUID:UUID] = [:]
        let now = try #require(KudzuDate.parse("2026-11-01T00:00:00Z"))
        for (i,text) in fixture.texts.enumerated() {
            let classified = try #require(Clipboard.classify([[(Clipboard.uti.PLAIN_TEXT,Data(text.utf8))]]))
            let e = history.ingest(classified,now:now.addingTimeInterval(Double(i)*3600)).entry
            let expected = fixture.states[i]; ids[e.id] = expected.entries[0].id
            var normalized = history
            for j in normalized.entries.indices { normalized.entries[j].id = ids[normalized.entries[j].id] ?? normalized.entries[j].id }
            #expect(normalized == expected)
        }
    }
    struct SyncCase: Decodable {
        var library: Library; var index: Sync.SyncIndex; var records: [Sync.RemoteRecord]; var deleted: [String]
        var expectedLibrary: Library; var expectedIndex: Sync.SyncIndex; var libraryChanged: Bool; var needsUpdate: Bool; var changes: Sync.LocalChanges
    }
    @Test func syncFixtureReplay() throws {
        for c in try t2Fixture([SyncCase].self,"sync/merges.json") {
            var library = c.library, index = c.index
            let applied = Sync.applyRemote(library:&library,index:&index,records:c.records,deleted:c.deleted,deviceID:"mac-a")
            #expect(library == c.expectedLibrary); #expect(index == c.expectedIndex)
            #expect(applied.libraryChanged == c.libraryChanged); #expect(applied.needsUpdate == c.needsUpdate)
            #expect(Sync.localChanges(library:library,index:index,deviceID:"mac-a") == c.changes)
        }
    }
    struct ImportCase: Decodable { var source: String; var file: String?; var text: String?; var files: [String:String]?; var expected: ImportResult }
    @Test func interchangeFixtureReplay() throws {
        for c in try t2Fixture([ImportCase].self,"interchange/imports.json") {
            let actual: ImportResult
            if c.source == "raycast" { actual = try Raycast.importJSON(c.text ?? "",fileName:c.file ?? "") }
            else if c.source == "textexpander" { actual = try TextExpander.importData(Data((c.text ?? "").utf8),fileName:c.file ?? "") }
            else {
                let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
                defer { try? FileManager.default.removeItem(at:root) }
                for (path,text) in c.files ?? [:] { let url = root.appendingPathComponent(path); try FileManager.default.createDirectory(at:url.deletingLastPathComponent(),withIntermediateDirectories:true); try Data(text.utf8).write(to:url) }
                actual = try Espanso.importDir(root)
            }
            #expect(actual.warnings == c.expected.warnings, "\(c.source): \(actual.warnings)")
            #expect(actual.library.groups.count == c.expected.library.groups.count); #expect(actual.library.snippets.count == c.expected.library.snippets.count)
            var ids: [UUID:UUID] = [:]
            for (a,b) in zip(actual.library.groups,c.expected.library.groups) { ids[a.id] = b.id }
            for (a,b) in zip(actual.library.snippets,c.expected.library.snippets) { ids[a.id] = b.id }
            var library = actual.library
            for i in library.groups.indices { library.groups[i].id = ids[library.groups[i].id] ?? library.groups[i].id; library.groups[i].parentID = library.groups[i].parentID.map { ids[$0] ?? $0 } }
            for i in library.snippets.indices {
                library.snippets[i].id = ids[library.snippets[i].id] ?? library.snippets[i].id
                library.snippets[i].groupID = library.snippets[i].groupID.map { ids[$0] ?? $0 }
                for (a,b) in ids { library.snippets[i].content = library.snippets[i].content.replacingOccurrences(of:a.uuidString,with:b.uuidString) }
                let expected = c.expected.library.snippets[i]
                // Import-generated timestamps vary per run. Source-defined TE dates stay asserted.
                if c.source != "textexpander" || expected.createdAt > Date(timeIntervalSince1970:1_760_000_000) { library.snippets[i].createdAt = expected.createdAt; library.snippets[i].updatedAt = expected.updatedAt }
            }
            #expect(library == c.expected.library,"\(c.source)")
        }
    }
}

extension T2FixtureTests {
    struct YAMLCase: Decodable { var text: String; var expected: ImportResult }
    @Test func yamlScalarFixtureReplay() throws {
        let dir = try t2Temp(); defer { try? FileManager.default.removeItem(at:dir) }; let path = dir.appendingPathComponent("case.yml")
        for c in try t2Fixture([YAMLCase].self,"interchange/yaml.json") {
            try Data(c.text.utf8).write(to:path); let r = Espanso.importFiles([path])
            #expect(r.warnings == c.expected.warnings,"\(c.text)"); #expect(r.library.snippets.map(\.content) == c.expected.library.snippets.map(\.content),"\(c.text)")
        }
    }
}
