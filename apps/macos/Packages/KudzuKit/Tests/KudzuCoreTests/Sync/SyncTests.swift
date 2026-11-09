import Foundation
import Testing
@testable import KudzuCore

@Suite struct SyncTests {
    let me = "mac-a", other = "mac-b"
    func id(_ n: Int) -> UUID { UUID(uuidString:String(format:"00000000-0000-0000-0000-%012d",n))! }
    func snippet(_ n: Int, _ trigger: String, _ minutes: Int = 0) -> Snippet { Snippet(id:id(n),name:"S\(n)",triggers:[trigger],content:"content \(n)",createdAt:Date(timeIntervalSince1970:1_780_000_000+Double(minutes)*60)) }
    func send(_ lib: Library, _ idx: inout Sync.SyncIndex, _ changes: Sync.LocalChanges) throws {
        for name in changes.saves { Sync.recordSaved(index:&idx,record:try #require(Sync.encode(name,library:lib,index:idx,deviceID:me))) }
        for name in changes.deletes { Sync.recordDeleted(index:&idx,name:name) }
    }
    func record(_ s: Snippet) throws -> Sync.RemoteRecord { Sync.RemoteRecord(kind:.snippet,name:Sync.snippetName(s.id),payload:String(decoding:try KudzuJSON.encode(s),as:UTF8.self)) }
    @Test func firstSyncUploadsEverythingThenNothing() throws {
        let lib = Library(snippets:[snippet(1,";a"),snippet(2,";b")]); var idx = Sync.SyncIndex()
        let c = Sync.localChanges(library:lib,index:idx,deviceID:me); #expect(c.saves == [Sync.snippetName(id(1)),Sync.snippetName(id(2)),"meta"])
        try send(lib,&idx,c); #expect(Sync.localChanges(library:lib,index:idx,deviceID:me).isEmpty)
    }
    @Test func editsAndDeletionsAreDetected() throws {
        var lib = Library(snippets:[snippet(1,";a"),snippet(2,";b")]), idx = Sync.SyncIndex()
        try send(lib,&idx,Sync.localChanges(library:lib,index:idx,deviceID:me)); lib.snippets[0].content = "edited"; lib.snippets.removeLast()
        let c = Sync.localChanges(library:lib,index:idx,deviceID:me); #expect(c.saves == [Sync.snippetName(id(1))]); #expect(c.deletes == [Sync.snippetName(id(2))])
    }
    @Test func unknownFieldsFromNewerVersionsSurviveAnEdit() throws {
        var lib = Library(snippets:[snippet(1,";a")]), idx = Sync.SyncIndex(), json = Sync.object(snippet(1,";a")); json["color"] = "green"
        let remote = Sync.RemoteRecord(kind:.snippet,name:Sync.snippetName(id(1)),payload:try #require(Sync.json(json)))
        _ = Sync.applyRemote(library:&lib,index:&idx,records:[remote],deleted:[],deviceID:me); lib.snippets[0].content = "edited here"
        let sent = try #require(Sync.encode(remote.name,library:lib,index:idx,deviceID:me)), v = try #require(Sync.parsed(sent.payload) as? [String:Any]); #expect(v["color"] as? String == "green"); #expect(v["content"] as? String == "edited here")
    }
    @Test func clearingAnOptionalFieldSyncs() throws {
        var s = snippet(1,";a"); s.groupID = id(9); var lib = Library(snippets:[s]), idx = Sync.SyncIndex()
        try send(lib,&idx,Sync.localChanges(library:lib,index:idx,deviceID:me)); lib.snippets[0].groupID = nil
        let rec = try #require(Sync.encode(Sync.snippetName(s.id),library:lib,index:idx,deviceID:me)), v = try #require(Sync.parsed(rec.payload) as? [String:Any]); #expect(v["groupID"] == nil)
    }
    @Test func remoteChangesApplyWithoutEchoingBack() throws {
        var lib = Library(snippets:[snippet(1,";a")]), idx = Sync.SyncIndex(); try send(lib,&idx,Sync.localChanges(library:lib,index:idx,deviceID:me))
        var theirs = snippet(1,";a",5); theirs.content = "from the other Mac"
        let a = Sync.applyRemote(library:&lib,index:&idx,records:[try record(theirs)],deleted:[],deviceID:me)
        #expect(a.libraryChanged); #expect(lib.snippets[0].content == theirs.content); #expect(Sync.localChanges(library:lib,index:idx,deviceID:me).isEmpty)
    }
    @Test func newerLocalEditWinsAConflictOlderOneLoses() throws {
        for minutes in [10,2] {
            var lib = Library(snippets:[snippet(1,";a")]), idx = Sync.SyncIndex(); try send(lib,&idx,Sync.localChanges(library:lib,index:idx,deviceID:me))
            lib.snippets[0].content = "mine"; lib.snippets[0].updatedAt.addTimeInterval(Double(minutes)*60)
            var theirs = snippet(1,";a",5); theirs.content = "theirs"
            _ = Sync.applyRemote(library:&lib,index:&idx,records:[try record(theirs)],deleted:[],deviceID:me)
            #expect(lib.snippets[0].content == (minutes == 10 ? "mine" : "theirs"))
            if minutes == 10 { #expect(Sync.localChanges(library:lib,index:idx,deviceID:me).saves == [Sync.snippetName(id(1))]) }
        }
    }
    @Test func remoteGroupDeletionKeepsItsSnippets() throws {
        let g = Group(id:id(50),name:"Work"); var s = snippet(1,";a"); s.groupID = g.id
        var lib = Library(groups:[g],snippets:[s]), idx = Sync.SyncIndex(); try send(lib,&idx,Sync.localChanges(library:lib,index:idx,deviceID:me))
        let a = Sync.applyRemote(library:&lib,index:&idx,records:[],deleted:[Sync.groupName(g.id)],deviceID:me)
        #expect(a.libraryChanged); #expect(lib.groups.isEmpty); #expect(lib.snippets.count == 1); #expect(lib.snippets[0].groupID == nil)
    }
    @Test func usageIsPerMacAndNeverOverwritten() throws {
        var lib = Library(snippets:[snippet(1,";a")],usage:[UsageRecord(snippetID:id(1),deviceID:me,count:3,charsSaved:30)]), idx = Sync.SyncIndex()
        try send(lib,&idx,Sync.localChanges(library:lib,index:idx,deviceID:me))
        let payload = try #require(Sync.json(["deviceID":other,"rows":[Sync.object(UsageRecord(snippetID:id(1),deviceID:other,count:7,charsSaved:70))]]))
        _ = Sync.applyRemote(library:&lib,index:&idx,records:[.init(kind:.usage,name:Sync.usageName(other),payload:payload)],deleted:[],deviceID:me)
        #expect(lib.usage.reduce(0) { $0+$1.count } == 10); #expect(Sync.localChanges(library:lib,index:idx,deviceID:me).isEmpty)
        lib.usage.removeAll { $0.deviceID == other }; #expect(Sync.localChanges(library:lib,index:idx,deviceID:me).deletes.isEmpty)
    }
    @Test func aNewerSyncFormatStopsSending() {
        var lib = Library(snippets:[snippet(1,";a")]), idx = Sync.SyncIndex()
        let a = Sync.applyRemote(library:&lib,index:&idx,records:[.init(kind:.meta,name:"meta",payload:#"{"syncVersion":2}"#)],deleted:[],deviceID:me)
        #expect(a.needsUpdate); lib.snippets[0].content = "edited"; #expect(Sync.localChanges(library:lib,index:idx,deviceID:me).isEmpty)
    }
    @Test func metaIsWrittenOnceAndNeverDowngraded() throws {
        let lib = Library(); var idx = Sync.SyncIndex(); let c = Sync.localChanges(library:lib,index:idx,deviceID:me)
        #expect(c.saves == ["meta"]); try send(lib,&idx,c); #expect(idx.serverSyncVersion == Sync.SYNC_VERSION); #expect(Sync.localChanges(library:lib,index:idx,deviceID:me).isEmpty)
    }
    @Test func initialMergeFoldsDuplicateStarterSnippets() {
        var lib = Library(snippets:[snippet(1,";kudzu"),snippet(2,";kudzu",1)]), idx = Sync.SyncIndex(); lib.snippets[1].content = lib.snippets[0].content
        #expect(Sync.initialMerge(library:&lib,index:&idx)); #expect(lib.snippets.count == 1); #expect(!Sync.initialMerge(library:&lib,index:&idx))
    }
    @Test func recordsForMissingItemsAreSkipped() { #expect(Sync.encode(Sync.snippetName(id(1)),library:.empty,index:.init(),deviceID:me) == nil); #expect(Sync.RecordKind.fromRecordType("KudzuSnippet") == .snippet); #expect(Sync.RecordKind.fromRecordType("Nope") == nil) }

    @Test func signedOutDeletionDoesNotResurrectAndQueuesADelete() throws {
        var lib = Library(), idx = Sync.SyncIndex()
        let remote = snippet(1, ";a")
        let name = Sync.snippetName(remote.id)
        let kept = snippet(2, ";b")
        lib.snippets = [kept]
        let applied = Sync.applyRemote(library: &lib, index: &idx, records: [try record(remote), try record(kept)], deleted: [], deviceID: me, locallyDeleted: [name])
        #expect(!applied.libraryChanged)
        #expect(lib.snippets.map(\.id) == [kept.id])
        #expect(idx.records[name]?.kind == .snippet)
        let changes = Sync.localChanges(library: lib, index: idx, deviceID: me)
        #expect(changes.deletes == [name])
        #expect(!changes.saves.contains(name))
    }

    @Test func aKeptSnippetStillTakesTheRemoteUpdate() throws {
        var lib = Library(snippets: [snippet(1, ";a")]), idx = Sync.SyncIndex()
        try send(lib, &idx, Sync.localChanges(library: lib, index: idx, deviceID: me))
        var theirs = snippet(1, ";a", 5)
        theirs.content = "from the other Mac"
        let name = Sync.snippetName(id(1))
        _ = Sync.applyRemote(library: &lib, index: &idx, records: [try record(theirs)], deleted: [], deviceID: me, locallyDeleted: [name])
        #expect(lib.snippets[0].content == "from the other Mac")
    }

    @Test func remoteDeletionOfAnUnsentEditKeepsTheLocalSnippet() throws {
        var lib = Library(snippets: [snippet(1, ";a")]), idx = Sync.SyncIndex()
        try send(lib, &idx, Sync.localChanges(library: lib, index: idx, deviceID: me))
        lib.snippets[0].content = "unsent"
        lib.snippets[0].updatedAt = Date(timeIntervalSince1970: 1_900_000_000)
        let name = Sync.snippetName(id(1))
        let applied = Sync.applyRemote(library: &lib, index: &idx, records: [], deleted: [name], deviceID: me)
        #expect(!applied.libraryChanged)
        #expect(lib.snippets.map(\.content) == ["unsent"])
        #expect(idx.records[name] == nil)
        let changes = Sync.localChanges(library: lib, index: idx, deviceID: me)
        #expect(changes.saves.contains(name))
        #expect(!changes.deletes.contains(name))
    }

    @Test func remoteDeletionWithNoIndexEntryStillRemovesTheSnippet() throws {
        var lib = Library(snippets: [snippet(1, ";a")]), idx = Sync.SyncIndex()
        let name = Sync.snippetName(id(1))
        let applied = Sync.applyRemote(library: &lib, index: &idx, records: [], deleted: [name], deviceID: me)
        #expect(applied.libraryChanged)
        #expect(lib.snippets.isEmpty)
    }

    @Test func namesAbsentKeepsNamesTheLibraryStillHas() {
        let kept = snippet(1, ";a")
        let lib = Library(snippets: [kept])
        let gone = Sync.snippetName(id(2))
        #expect(Sync.namesAbsent(from: lib, names: [Sync.snippetName(kept.id), gone, "not-a-record"]) == Set([gone, "not-a-record"]))
        #expect(Sync.tombstoneKind(gone) == .snippet)
        #expect(Sync.tombstoneKind("not-a-record") == nil)
        #expect(Sync.tombstoneKind("group-" + id(3).uuidString.lowercased()) == .group)
    }
}
