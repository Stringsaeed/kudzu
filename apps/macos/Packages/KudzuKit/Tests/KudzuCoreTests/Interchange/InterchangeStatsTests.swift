import Foundation
import Testing
@testable import KudzuCore

@Suite struct InterchangeStatsTests {
    let t0 = Date(timeIntervalSince1970:1_000_000_000)
    func snippets() -> [Snippet] { [Snippet(name:"A",triggers:[";a"],content:"alpha"),Snippet(name:"B",triggers:[";b"],content:"beta"),Snippet(name:"C",triggers:[";c"],content:"gamma")] }
    func row(_ id: UUID, _ device: String, _ count: Int, _ chars: Int, _ date: Date?) -> UsageRecord { UsageRecord(snippetID:id,deviceID:device,count:count,charsSaved:chars,lastUsedAt:date) }
    @Test func sumsAcrossDevicesAndSortsByCountDescending() {
        let a = snippets(), usage = [row(a[0].id,"mac1",2,20,t0),row(a[0].id,"mac2",3,30,t0.addingTimeInterval(60)),row(a[1].id,"mac1",10,5,t0)], s = InterchangeStats.summary(usage:usage,snippets:a,typingSpeedWPM:40)
        #expect(s.perSnippet.map(\.snippetID) == [a[1].id,a[0].id,a[2].id]); #expect(s.perSnippet[1].count == 5); #expect(s.perSnippet[1].charsSaved == 50); #expect(s.perSnippet[1].lastUsedAt == t0.addingTimeInterval(60)); #expect(s.perSnippet[2].count == 0); #expect(s.perSnippet[2].lastUsedAt == nil); #expect(s.totalExpansions == 15); #expect(s.totalCharsSaved == 55)
    }
    @Test func estimatesTimeSavedFromWpm() { let a = snippets(), rows = [row(a[0].id,"m",1,1000,nil)]; #expect(InterchangeStats.summary(usage:rows,snippets:[a[0]],typingSpeedWPM:40).estimatedTimeSaved == 300); #expect(InterchangeStats.summary(usage:rows,snippets:[a[0]],typingSpeedWPM:0).estimatedTimeSaved == 0) }
    @Test func ignoresUsageForUnknownSnippets() { let s = InterchangeStats.summary(usage:[row(UUID(),"m",7,70,nil)],snippets:[snippets()[0]],typingSpeedWPM:40); #expect(s.totalExpansions == 0); #expect(s.perSnippet.count == 1) }
    @Test func topSnippetsSkipsUnusedAndLimits() { let a = snippets(), s = InterchangeStats.summary(usage:[row(a[0].id,"m",1,0,nil),row(a[1].id,"m",2,0,nil)],snippets:a,typingSpeedWPM:40); #expect(s.topSnippets(1).map(\.snippetID) == [a[1].id]); #expect(s.topSnippets(10).map(\.snippetID) == [a[1].id,a[0].id]) }
    @Test func tiesAreBrokenDeterministically() { let a = snippets(), usage = [row(a[0].id,"m",1,1,t0),row(a[1].id,"m",1,1,t0)]; #expect(InterchangeStats.summary(usage:usage,snippets:Array(a.prefix(2)),typingSpeedWPM:40).perSnippet == InterchangeStats.summary(usage:usage.reversed(),snippets:Array(a.prefix(2).reversed()),typingSpeedWPM:40).perSnippet) }
    @Test func usageRankFormula() {
        #expect(InterchangeStats.usageRank(count:0,lastUsedAt:nil,now:t0) == 0)
        let fresh = InterchangeStats.usageRank(count:1,lastUsedAt:t0,now:t0), week = InterchangeStats.usageRank(count:1,lastUsedAt:t0,now:t0.addingTimeInterval(7*86400)), future = InterchangeStats.usageRank(count:1,lastUsedAt:t0.addingTimeInterval(3600),now:t0)
        #expect(abs(fresh-(log(2)+2)) < 1e-9); #expect(abs(week-(log(2)+1)) < 1e-9); #expect(abs(future-fresh) < 1e-9)
    }
    @Test func recentBeatsSlightlyMoreFrequentButStale() { let a = snippets(), now = t0.addingTimeInterval(60*86400), usage = [row(a[0].id,"m",12,0,t0),row(a[1].id,"m",8,0,now)], s = InterchangeStats.summary(usage:usage,snippets:Array(a.prefix(2)),typingSpeedWPM:40); #expect(s.usageRankFor(a[1].id,now:now) > s.usageRankFor(a[0].id,now:now)); #expect(s.usageRankFor(a[2].id,now:now) == 0); #expect(s.usageRanks(now:now).count == 2) }
}
