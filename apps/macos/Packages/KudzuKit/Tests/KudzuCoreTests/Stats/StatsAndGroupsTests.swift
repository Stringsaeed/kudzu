import Foundation
import Testing
@testable import KudzuCore

@Test func usageAggregatesDevicesAndClampsNegativeSavings() {
    let s = Snippet(name:"S"), now = Date(timeIntervalSince1970:100)
    let library = Library(snippets:[s],usage:[UsageRecord(snippetID:s.id,deviceID:"a",count:2,charsSaved:-3,lastUsedAt:now),UsageRecord(snippetID:s.id,deviceID:"b",count:3,charsSaved:500)])
    let usage = Stats.usageById(library), totals = Stats.totals(library,usage:usage,wpm:50)
    #expect(usage[s.id] == SnippetUsage(count:5,chars:500,last:100000))
    #expect(totals.expansions == 5 && totals.chars == 500 && totals.minutesSaved == 2 && totals.activeSnippets == 1 && totals.usedSnippets == 1)
}
@Test func countsDurationsAndRelativeTimes() {
    #expect([0,999,1000,9999,10000,1_000_000].map(Stats.formatCount) == ["0","999","1.0k","10.0k","10k","1.0M"])
    #expect([0.5,1,59.5,60,600,1440].map(Stats.formatDuration) == ["30s","1 min","60 min","1.0 h","10 h","1.0 days"])
    #expect(Stats.relativeTime(0) == "Never")
    #expect(Stats.relativeTime(100000,now:Date(timeIntervalSince1970:110)) == "Just now")
    #expect(Stats.CHARS_PER_WORD == 5)
}
@Test func groupsRespectOrderCollapseCyclesAndInheritance() {
    let top = Group(name:"Top",isEnabled:false), child = Group(name:"Child",parentID:top.id)
    let dangling = Group(name:"Dangling",parentID:UUID())
    let groups = [child,top,dangling], tree = Groups.buildTree(groups)
    #expect(Groups.flatten(tree).map(\.group.name) == ["Dangling","Top","Child"])
    #expect(Groups.flatten(tree,collapsed:[top.id]).map(\.group.name) == ["Dangling","Top"])
    #expect(Groups.descendantIds(groups,id:top.id) == [top.id,child.id])
    #expect(Groups.groupPath(groups,id:child.id) == ["Top","Child"])
    #expect(!Groups.isGroupEnabled(groups,id:child.id))
    #expect(Groups.recursiveCounts(groups,snippets:[Snippet(groupID:child.id)]) == [child.id:1,top.id:1])
    let id = UUID(), cycle = Group(id:id,name:"Self",parentID:id)
    #expect(Groups.flatten(Groups.buildTree([cycle])).count == 1)
}
@Test func durationRoundingMatchesJavaScriptToFixed() {
    #expect(Stats.formatDuration(75) == "1.3 h")
    #expect(Stats.formatDuration(153) == "2.5 h")
    #expect(Stats.formatCount(1250) == "1.3k")
    #expect(Stats.formatCount(2550) == "2.5k")
}
