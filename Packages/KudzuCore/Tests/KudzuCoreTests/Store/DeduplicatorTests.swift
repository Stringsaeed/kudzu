import Foundation
import Testing
@testable import KudzuCore

/// Deterministic UUIDs so "smallest UUID string" tie-breaks are predictable.
func uuid(_ n: Int) -> UUID {
    UUID(uuidString: String(format: "00000000-0000-0000-0000-%012d", n))!
}

let day0 = Date(timeIntervalSinceReferenceDate: 800_000_000)

@Suite struct DeduplicatorTests {
    @Test func emptyLibraryHasEmptyPlan() {
        let plan = Deduplicator.plan(for: KudzuLibrary())
        #expect(plan.isEmpty)
        #expect(plan.conflicts.isEmpty)
    }

    @Test func mergesSnippetsWithSameNormalizedTriggersAndContent() {
        let older = SnippetRecord(id: uuid(9), name: "Sig", triggers: [";sig", ";s"], content: "Best", isFavorite: false, isEnabled: false, createdAt: day0, updatedAt: day0)
        let newer = SnippetRecord(id: uuid(1), name: "Sig copy", triggers: [";S", ";SIG"], content: "Best", isFavorite: true, isEnabled: true, createdAt: day0.addingTimeInterval(10), updatedAt: day0.addingTimeInterval(20))
        let library = KudzuLibrary(snippets: [newer, older])

        let plan = Deduplicator.plan(for: library)

        #expect(plan.snippetMerges == [newer.id: older.id])
        let survivor = try! #require(plan.updatedSnippets.first)
        #expect(survivor.id == older.id)
        #expect(survivor.name == "Sig")
        #expect(survivor.triggers == [";sig", ";s"])
        #expect(survivor.isFavorite)
        #expect(survivor.isEnabled)
        #expect(survivor.createdAt == day0)
        #expect(survivor.updatedAt == day0.addingTimeInterval(20))

        let result = plan.applied(to: library)
        #expect(result.snippets.map(\.id) == [older.id])
    }

    @Test func createdAtTieBreaksOnSmallestUUIDString() {
        let a = SnippetRecord(id: uuid(2), name: "a", triggers: [";x"], content: "X", createdAt: day0)
        let b = SnippetRecord(id: uuid(1), name: "b", triggers: [";x"], content: "X", createdAt: day0)
        let plan = Deduplicator.plan(for: KudzuLibrary(snippets: [a, b]))
        #expect(plan.snippetMerges == [a.id: b.id])
    }

    @Test func duplicateTriggersWithinASnippetAreASet() {
        let a = SnippetRecord(id: uuid(1), name: "a", triggers: [";x", ";x"], content: "X", createdAt: day0)
        let b = SnippetRecord(id: uuid(2), name: "b", triggers: [";X"], content: "X", createdAt: day0)
        #expect(Deduplicator.plan(for: KudzuLibrary(snippets: [a, b])).snippetMerges == [b.id: a.id])
    }

    @Test func differentContentIsNotMergedButReportedAsConflict() {
        let a = SnippetRecord(id: uuid(1), name: "a", triggers: [";addr", ";home"], content: "1 Main St", createdAt: day0)
        let b = SnippetRecord(id: uuid(2), name: "b", triggers: [";ADDR"], content: "2 Side St", createdAt: day0)
        let c = SnippetRecord(id: uuid(3), name: "c", triggers: [";other"], content: "x", createdAt: day0)
        let plan = Deduplicator.plan(for: KudzuLibrary(snippets: [b, c, a]))

        #expect(plan.isEmpty)
        #expect(plan.conflicts == [DedupeConflict(trigger: ";addr", snippetIDs: [a.id, b.id])])
    }

    @Test func contentComparisonIsExact() {
        let a = SnippetRecord(id: uuid(1), name: "a", triggers: [";x"], content: "Hello", createdAt: day0)
        let b = SnippetRecord(id: uuid(2), name: "b", triggers: [";x"], content: "hello", createdAt: day0)
        let plan = Deduplicator.plan(for: KudzuLibrary(snippets: [a, b]))
        #expect(plan.snippetMerges.isEmpty)
        #expect(plan.conflicts.count == 1)
    }

    @Test func usageOfMergedSnippetsIsRepointedAndSummedPerDevice() {
        let keep = SnippetRecord(id: uuid(1), name: "k", triggers: [";k"], content: "K", createdAt: day0)
        let drop = SnippetRecord(id: uuid(2), name: "d", triggers: [";k"], content: "K", createdAt: day0.addingTimeInterval(1))
        let usage = [
            UsageRecord(snippetID: keep.id, deviceID: "macA", count: 3, charsSaved: 30, lastUsedAt: day0),
            UsageRecord(snippetID: drop.id, deviceID: "macA", count: 2, charsSaved: 20, lastUsedAt: day0.addingTimeInterval(100)),
            UsageRecord(snippetID: drop.id, deviceID: "macB", count: 5, charsSaved: 50, lastUsedAt: nil),
        ]
        let library = KudzuLibrary(snippets: [keep, drop], usage: usage)
        let plan = Deduplicator.plan(for: library)

        #expect(plan.usageMerges == [
            UsageMerge(snippetID: keep.id, deviceID: "macA", sourceSnippetIDs: [keep.id, drop.id], count: 5, charsSaved: 50, lastUsedAt: day0.addingTimeInterval(100)),
            UsageMerge(snippetID: keep.id, deviceID: "macB", sourceSnippetIDs: [drop.id], count: 5, charsSaved: 50, lastUsedAt: nil),
        ])
        let result = plan.applied(to: library)
        #expect(Set(result.usage) == [
            UsageRecord(snippetID: keep.id, deviceID: "macA", count: 5, charsSaved: 50, lastUsedAt: day0.addingTimeInterval(100)),
            UsageRecord(snippetID: keep.id, deviceID: "macB", count: 5, charsSaved: 50),
        ])
    }

    @Test func duplicateUsageRowsForSameSnippetAndDeviceAreSummed() {
        let s = SnippetRecord(id: uuid(1), name: "s", triggers: [";s"], content: "S", createdAt: day0)
        let library = KudzuLibrary(snippets: [s], usage: [
            UsageRecord(snippetID: s.id, deviceID: "m", count: 1, charsSaved: 1),
            UsageRecord(snippetID: s.id, deviceID: "m", count: 2, charsSaved: 2),
        ])
        let result = Deduplicator.plan(for: library).applied(to: library)
        #expect(result.usage == [UsageRecord(snippetID: s.id, deviceID: "m", count: 3, charsSaved: 3)])
    }

    @Test func orphanUsageIsLeftAloneUnlessAnAliasPointsSomewhere() {
        let s = SnippetRecord(id: uuid(1), name: "s", triggers: [";s"], content: "S", createdAt: day0)
        let ghost = uuid(50), aliased = uuid(51)
        let library = KudzuLibrary(snippets: [s], usage: [
            UsageRecord(snippetID: ghost, deviceID: "m", count: 1),
            UsageRecord(snippetID: aliased, deviceID: "m", count: 4),
        ])
        // The ghost row may belong to a snippet that hasn't synced yet: keep it.
        // The aliased one was merged into `s` on another device before its usage arrived.
        let plan = Deduplicator.plan(for: library, aliases: [aliased: s.id])
        #expect(plan.usageMerges == [UsageMerge(snippetID: s.id, deviceID: "m", sourceSnippetIDs: [aliased], count: 4, charsSaved: 0, lastUsedAt: nil)])
        #expect(plan.applied(to: library).usage.contains { $0.snippetID == ghost })
    }

    @Test func mergesGroupsByCaseInsensitiveNameUnderSameParent() {
        let work1 = GroupRecord(id: uuid(1), name: "Work")
        let work2 = GroupRecord(id: uuid(2), name: " work ")
        let personal = GroupRecord(id: uuid(3), name: "Personal")
        // Same name, different parent: not merged.
        let nestedWork = GroupRecord(id: uuid(4), name: "Work", parentID: personal.id)
        let s = SnippetRecord(id: uuid(10), name: "s", triggers: [";s"], content: "S", groupID: work2.id, createdAt: day0)
        let library = KudzuLibrary(groups: [work2, personal, work1, nestedWork], snippets: [s])

        let plan = Deduplicator.plan(for: library)
        #expect(plan.groupMerges == [work2.id: work1.id])
        #expect(plan.updatedSnippets.map(\.groupID) == [work1.id])

        let result = plan.applied(to: library)
        #expect(Set(result.groups.map(\.id)) == [work1.id, personal.id, nestedWork.id])
        #expect(result.snippets.first?.groupID == work1.id)
    }

    @Test func groupMergesCascadeIntoChildren() {
        // Two copies of Work/Email: after Work merges, the two Email groups share a parent and merge too.
        let workA = GroupRecord(id: uuid(1), name: "Work")
        let workB = GroupRecord(id: uuid(2), name: "Work")
        let emailA = GroupRecord(id: uuid(4), name: "Email", parentID: workA.id)
        let emailB = GroupRecord(id: uuid(3), name: "Email", parentID: workB.id)
        let leafB = GroupRecord(id: uuid(5), name: "Leaf", parentID: emailB.id)
        let library = KudzuLibrary(groups: [workA, workB, emailA, emailB, leafB])

        let plan = Deduplicator.plan(for: library)
        #expect(plan.groupMerges == [workB.id: workA.id, emailA.id: emailB.id])
        let result = plan.applied(to: library)
        #expect(result.groups.count == 3)
        #expect(result.groups.first { $0.id == emailB.id }?.parentID == workA.id)
        #expect(result.groups.first { $0.id == leafB.id }?.parentID == emailB.id)
    }

    @Test func isIdempotentAndOrderIndependent() {
        let library = Self.messyLibrary()
        let plan = Deduplicator.plan(for: library)
        let once = plan.applied(to: library)
        let secondPlan = Deduplicator.plan(for: once)
        #expect(secondPlan.isEmpty)
        #expect(secondPlan.applied(to: once) == once)

        let shuffled = KudzuLibrary(
            groups: library.groups.reversed(),
            snippets: library.snippets.reversed(),
            usage: library.usage.reversed()
        )
        let shuffledPlan = Deduplicator.plan(for: shuffled)
        #expect(shuffledPlan == plan)
    }

    /// Two Macs each imported the same Espanso config (fresh UUIDs, slightly
    /// different timestamps), used a few snippets, then synced.
    @Test func sameEspansoConfigImportedOnTwoMacs() {
        func importConfig(idBase: Int, at date: Date) -> KudzuLibrary {
            let group = GroupRecord(id: uuid(idBase), name: "base")
            let snippets = [
                SnippetRecord(id: uuid(idBase + 1), name: ":sig", triggers: [":sig"], content: "Regards", groupID: group.id, createdAt: date, updatedAt: date),
                SnippetRecord(id: uuid(idBase + 2), name: ":date", triggers: [":date"], content: "{{date}}", groupID: group.id, fireMode: .wholeWord, createdAt: date, updatedAt: date),
                SnippetRecord(id: uuid(idBase + 3), name: ":addr", triggers: [":addr", ":address"], content: "1 Main St", groupID: group.id, createdAt: date, updatedAt: date),
            ]
            return KudzuLibrary(groups: [group], snippets: snippets)
        }
        let macA = importConfig(idBase: 100, at: day0)
        let macB = importConfig(idBase: 200, at: day0.addingTimeInterval(3600))
        let usage = [
            UsageRecord(snippetID: uuid(101), deviceID: "A", count: 4, charsSaved: 28, lastUsedAt: day0.addingTimeInterval(10)),
            UsageRecord(snippetID: uuid(201), deviceID: "B", count: 6, charsSaved: 42, lastUsedAt: day0.addingTimeInterval(7200)),
            UsageRecord(snippetID: uuid(203), deviceID: "B", count: 1, charsSaved: 9, lastUsedAt: day0.addingTimeInterval(7300)),
        ]
        let synced = KudzuLibrary(groups: macA.groups + macB.groups, snippets: macA.snippets + macB.snippets, usage: usage)

        let plan = Deduplicator.plan(for: synced)
        #expect(plan.groupMerges == [uuid(200): uuid(100)])
        #expect(plan.snippetMerges == [uuid(201): uuid(101), uuid(202): uuid(102), uuid(203): uuid(103)])
        #expect(plan.conflicts.isEmpty)

        let result = plan.applied(to: synced)
        #expect(result.groups.map(\.id) == [uuid(100)])
        #expect(Set(result.snippets.map(\.id)) == [uuid(101), uuid(102), uuid(103)])
        #expect(result.snippets.allSatisfy { $0.groupID == uuid(100) })

        // Stats are preserved across the merge.
        let before = StatsCalculator.summary(usage: synced.usage, snippets: synced.snippets, typingSpeedWPM: 40)
        let after = StatsCalculator.summary(usage: result.usage, snippets: result.snippets, typingSpeedWPM: 40)
        #expect(after.totalExpansions == before.totalExpansions)
        #expect(after.totalCharsSaved == before.totalCharsSaved)
        #expect(Set(result.usage) == [
            UsageRecord(snippetID: uuid(101), deviceID: "A", count: 4, charsSaved: 28, lastUsedAt: day0.addingTimeInterval(10)),
            UsageRecord(snippetID: uuid(101), deviceID: "B", count: 6, charsSaved: 42, lastUsedAt: day0.addingTimeInterval(7200)),
            UsageRecord(snippetID: uuid(103), deviceID: "B", count: 1, charsSaved: 9, lastUsedAt: day0.addingTimeInterval(7300)),
        ])

        // Both Macs compute the same plan from the same synced data, so they converge.
        let fromOtherMac = Deduplicator.plan(for: KudzuLibrary(groups: macB.groups + macA.groups, snippets: macB.snippets + macA.snippets, usage: usage.reversed()))
        #expect(fromOtherMac == plan)
        #expect(Deduplicator.plan(for: result).isEmpty)
    }

    static func messyLibrary() -> KudzuLibrary {
        let g1 = GroupRecord(id: uuid(1), name: "A")
        let g2 = GroupRecord(id: uuid(2), name: "a")
        let g3 = GroupRecord(id: uuid(3), name: "B", parentID: g2.id)
        let g4 = GroupRecord(id: uuid(4), name: "b", parentID: g1.id)
        let s = [
            SnippetRecord(id: uuid(10), name: "1", triggers: [";x"], content: "X", groupID: g3.id, createdAt: day0),
            SnippetRecord(id: uuid(11), name: "2", triggers: [";X"], content: "X", groupID: g4.id, isFavorite: true, createdAt: day0.addingTimeInterval(-5)),
            SnippetRecord(id: uuid(12), name: "3", triggers: [";x"], content: "X", groupID: g2.id, isEnabled: false, createdAt: day0),
            SnippetRecord(id: uuid(13), name: "4", triggers: [";x"], content: "Y", createdAt: day0),
            SnippetRecord(id: uuid(14), name: "5", triggers: [";y"], content: "Y", groupID: g2.id, createdAt: day0),
        ]
        let u = [
            UsageRecord(snippetID: uuid(10), deviceID: "d1", count: 1, charsSaved: 1, lastUsedAt: day0),
            UsageRecord(snippetID: uuid(11), deviceID: "d1", count: 2, charsSaved: 2, lastUsedAt: day0.addingTimeInterval(9)),
            UsageRecord(snippetID: uuid(12), deviceID: "d2", count: 3, charsSaved: 3),
            UsageRecord(snippetID: uuid(14), deviceID: "d2", count: 1, charsSaved: 1),
        ]
        return KudzuLibrary(groups: [g1, g2, g3, g4], snippets: s, usage: u)
    }
}
