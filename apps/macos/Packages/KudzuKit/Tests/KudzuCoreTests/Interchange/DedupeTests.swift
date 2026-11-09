import Foundation
import Testing
@testable import KudzuCore

@Suite struct DedupeTests {
    func id(_ n: Int) -> UUID { UUID(uuidString:String(format:"00000000-0000-0000-0000-%012d",n))! }
    let day = Date(timeIntervalSince1970:1_778_307_200)
    func s(_ n: Int, _ ts: [String], _ content: String, _ seconds: Double = 0) -> Snippet { Snippet(id:id(n),name:String(n),triggers:ts,content:content,createdAt:day.addingTimeInterval(seconds)) }
    func g(_ n: Int, _ name: String, _ parent: Int? = nil) -> Group { Group(id:id(n),name:name,parentID:parent.map(id)) }
    func u(_ n: Int, _ device: String, _ count: Int, _ chars: Int = 0, _ date: Date? = nil) -> UsageRecord { UsageRecord(snippetID:id(n),deviceID:device,count:count,charsSaved:chars,lastUsedAt:date) }
    @Test func emptyLibraryHasEmptyPlan() { let p = Dedupe.plan(.empty); #expect(p.isEmpty); #expect(p.conflicts.isEmpty) }
    @Test func mergesSnippetsWithSameNormalizedTriggersAndContent() {
        var older = s(9,[";sig",";s"],"Best"); older.name = "Sig"; older.isEnabled = false
        var newer = s(1,[";S",";SIG"],"Best",10); newer.isFavorite = true; newer.updatedAt = day.addingTimeInterval(20)
        let lib = Library(snippets:[newer,older]), p = Dedupe.plan(lib), survivor = p.updatedSnippets[0]
        #expect(p.snippetMerges == [newer.id:older.id]); #expect(survivor.id == older.id); #expect(survivor.name == "Sig"); #expect(survivor.triggers == [";sig",";s"]); #expect(survivor.isFavorite && survivor.isEnabled); #expect(survivor.createdAt == day); #expect(survivor.updatedAt == day.addingTimeInterval(20)); #expect(p.applied(lib).snippets.map(\.id) == [older.id])
    }
    @Test func createdAtTieBreaksOnSmallestUuid() { #expect(Dedupe.plan(Library(snippets:[s(2,[";x"],"X"),s(1,[";x"],"X")])).snippetMerges == [id(2):id(1)]) }
    @Test func duplicateTriggersWithinASnippetAreASet() { #expect(Dedupe.plan(Library(snippets:[s(1,[";x",";x"],"X"),s(2,[";X"],"X")])).snippetMerges == [id(2):id(1)]) }
    @Test func differentContentIsReportedAsConflictNotMerged() {
        let p = Dedupe.plan(Library(snippets:[s(2,[";ADDR"],"2 Side St"),s(3,[";other"],"x"),s(1,[";addr",";home"],"1 Main St")]))
        #expect(p.isEmpty); #expect(p.conflicts == [DedupeConflict(trigger:";addr",snippetIds:[id(1),id(2)])])
    }
    @Test func contentComparisonIsExact() { let p = Dedupe.plan(Library(snippets:[s(1,[";x"],"Hello"),s(2,[";x"],"hello")])); #expect(p.snippetMerges.isEmpty); #expect(p.conflicts.count == 1) }
    @Test func usageOfMergedSnippetsIsRepointedAndSummedPerDevice() {
        let lib = Library(snippets:[s(1,[";k"],"K"),s(2,[";k"],"K",1)],usage:[u(1,"macA",3,30,day),u(2,"macA",2,20,day.addingTimeInterval(100)),u(2,"macB",5,50)])
        let p = Dedupe.plan(lib), expected = [u(1,"macA",5,50,day.addingTimeInterval(100)),u(1,"macB",5,50)]
        #expect(p.usageMerges.map(\.record) == expected); #expect(p.usageMerges[0].sourceSnippetIDs == [id(1),id(2)]); #expect(Set(p.applied(lib).usage) == Set(expected))
    }
    @Test func duplicateUsageRowsForSameSnippetAndDeviceAreSummed() { let result = Dedupe.dedupe(Library(snippets:[s(1,[";s"],"S")],usage:[u(1,"m",1,1),u(1,"m",2,2)])); #expect(result.library.usage == [u(1,"m",3,3)]); #expect(result.report.mergedUsageRows == 1) }
    @Test func orphanUsageIsLeftAloneUnlessAnAliasPointsSomewhere() {
        let lib = Library(snippets:[s(1,[";s"],"S")],usage:[u(50,"m",1),u(51,"m",4)]), p = Dedupe.plan(lib,aliases:[id(51):id(1)])
        #expect(p.usageMerges.count == 1); #expect(p.usageMerges[0].record == u(1,"m",4)); #expect(p.usageMerges[0].sourceSnippetIDs == [id(51)]); #expect(p.applied(lib).usage.contains { $0.snippetID == id(50) })
    }
    @Test func mergesGroupsByCaseInsensitiveNameUnderSameParent() {
        var snippet = s(10,[";s"],"S"); snippet.groupID = id(2)
        let lib = Library(groups:[g(2," work "),g(3,"Personal"),g(1,"Work"),g(4,"Work",3)],snippets:[snippet]), p = Dedupe.plan(lib)
        #expect(p.groupMerges == [id(2):id(1)]); #expect(p.updatedSnippets.map(\.groupID) == [id(1)]); #expect(Set(p.applied(lib).groups.map(\.id)) == [id(1),id(3),id(4)])
    }
    @Test func groupMergesCascadeIntoChildren() {
        let lib = Library(groups:[g(1,"Work"),g(2,"Work"),g(4,"Email",1),g(3,"Email",2),g(5,"Leaf",3)]), p = Dedupe.plan(lib), result = p.applied(lib)
        #expect(p.groupMerges == [id(2):id(1),id(4):id(3)]); #expect(result.groups.count == 3); #expect(result.groups.first { $0.id == id(3) }?.parentID == id(1)); #expect(result.groups.first { $0.id == id(5) }?.parentID == id(3))
    }
    func messy() -> Library {
        func grouped(_ n: Int, _ ts: [String], _ content: String, _ group: Int?, _ seconds: Double = 0) -> Snippet { var v = s(n,ts,content,seconds); v.groupID = group.map(id); return v }
        var fav = grouped(11,[";X"],"X",4,-5); fav.isFavorite = true; var disabled = grouped(12,[";x"],"X",2); disabled.isEnabled = false
        return Library(groups:[g(1,"A"),g(2,"a"),g(3,"B",2),g(4,"b",1)],snippets:[grouped(10,[";x"],"X",3),fav,disabled,s(13,[";x"],"Y"),grouped(14,[";y"],"Y",2)],usage:[u(10,"d1",1,1,day),u(11,"d1",2,2,day.addingTimeInterval(9)),u(12,"d2",3,3),u(14,"d2",1,1)])
    }
    @Test func isIdempotentAndOrderIndependent() { let lib = messy(), p = Dedupe.plan(lib), once = p.applied(lib); #expect(Dedupe.plan(once).isEmpty); #expect(Dedupe.plan(once).applied(once) == once); var reversed = lib; reversed.groups.reverse(); reversed.snippets.reverse(); reversed.usage.reverse(); #expect(Dedupe.plan(reversed) == p) }
    @Test func sameEspansoConfigImportedOnTwoMacsConverges() {
        func imported(_ base: Int, _ seconds: Double) -> (Group,[Snippet]) {
            let group = g(base,"base"); var snippets = [s(base+1,[":sig"],"Regards",seconds),s(base+2,[":date"],"{{date}}",seconds),s(base+3,[":addr",":address"],"1 Main St",seconds)]
            for i in snippets.indices { snippets[i].groupID = group.id }; snippets[1].fireMode = .wholeWord; return (group,snippets)
        }
        let (ga,sa) = imported(100,0), (gb,sb) = imported(200,3600), lib = Library(groups:[ga,gb],snippets:sa+sb,usage:[u(101,"A",4,28,day.addingTimeInterval(10)),u(201,"B",6,42,day.addingTimeInterval(7200)),u(203,"B",1,9,day.addingTimeInterval(7300))]), p = Dedupe.plan(lib), result = p.applied(lib)
        #expect(p.groupMerges == [id(200):id(100)]); #expect(p.snippetMerges == [id(201):id(101),id(202):id(102),id(203):id(103)]); #expect(p.conflicts.isEmpty); #expect(result.groups.map(\.id) == [id(100)]); #expect(result.snippets.allSatisfy { $0.groupID == id(100) })
        let before = InterchangeStats.summary(usage:lib.usage,snippets:lib.snippets,typingSpeedWPM:40), after = InterchangeStats.summary(usage:result.usage,snippets:result.snippets,typingSpeedWPM:40)
        #expect(after.totalExpansions == before.totalExpansions); #expect(after.totalCharsSaved == before.totalCharsSaved); #expect(Dedupe.plan(result).isEmpty)
    }
}
