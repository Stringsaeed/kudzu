import Foundation
import Testing
@testable import KudzuCore

@Suite("GroupTree")
struct GroupTreeTests {
    private func g(_ name: String, parent: UUID? = nil, sort: Int = 0, enabled: Bool = true,
                   rule: AppRule? = nil, insertion: InsertionMethod? = nil, id: UUID = UUID()) -> GroupRecord {
        GroupRecord(id: id, name: name, parentID: parent, sortIndex: sort, isEnabled: enabled, insertion: insertion, rule: rule)
    }

    @Test func buildsNestedTreeSortedBySortIndexThenName() {
        let a = g("A", sort: 1), b = g("B", sort: 0), c = g("c", sort: 0)
        let a1 = g("A1", parent: a.id, sort: 5), a0 = g("A0", parent: a.id, sort: 2)
        let tree = GroupTree(groups: [a, b, c, a1, a0])
        #expect(tree.roots.map(\.group.name) == ["B", "c", "A"])
        #expect(tree.roots[2].children.map(\.group.name) == ["A0", "A1"])
        #expect(tree.roots[0].optionalChildren == nil)
        #expect(tree.roots[2].optionalChildren?.count == 2)
    }

    @Test func danglingParentIsTopLevel() {
        let orphan = g("Orphan", parent: UUID())
        let tree = GroupTree(groups: [orphan])
        #expect(tree.roots.map(\.id) == [orphan.id])
        #expect(tree.parent(of: orphan.id) == nil)
    }

    @Test func cycleFromSyncIsBrokenAndEveryGroupAppearsOnce() {
        var a = g("A", sort: 0), b = g("B", sort: 1)
        a.parentID = b.id
        b.parentID = a.id
        let tree = GroupTree(groups: [a, b])
        #expect(Set(tree.allGroups.map(\.id)) == [a.id, b.id])
        #expect(tree.allGroups.count == 2)
        #expect(tree.roots.count == 1)
    }

    @Test func selfParentIsTopLevel() {
        var a = g("A")
        a.parentID = a.id
        let tree = GroupTree(groups: [a])
        #expect(tree.roots.map(\.id) == [a.id])
    }

    @Test func descendantsAndAncestors() {
        let a = g("A"), b = g("B", parent: a.id), c = g("C", parent: b.id), d = g("D")
        let tree = GroupTree(groups: [a, b, c, d])
        #expect(tree.descendantIDs(of: a.id) == [a.id, b.id, c.id])
        #expect(tree.descendantIDs(of: a.id, includingSelf: false) == [b.id, c.id])
        #expect(tree.ancestorIDs(of: c.id) == [b.id, a.id])
        #expect(tree.path(to: c.id) == ["A", "B", "C"])
        #expect(tree.descendantIDs(of: d.id, includingSelf: false).isEmpty)
    }

    @Test func recursiveCountsRollUpToAncestors() {
        let a = g("A"), b = g("B", parent: a.id)
        let tree = GroupTree(groups: [a, b])
        let snippets = [
            SnippetRecord(name: "1", triggers: [";1"], content: "", groupID: a.id),
            SnippetRecord(name: "2", triggers: [";2"], content: "", groupID: b.id),
            SnippetRecord(name: "3", triggers: [";3"], content: "", groupID: b.id),
            SnippetRecord(name: "4", triggers: [";4"], content: "", groupID: nil),
            SnippetRecord(name: "5", triggers: [";5"], content: "", groupID: UUID()),
        ]
        let counts = tree.recursiveSnippetCounts(snippets)
        #expect(counts[a.id] == 3)
        #expect(counts[b.id] == 2)
    }

    @Test func effectiveEnablementFollowsAncestors() {
        let a = g("A", enabled: false), b = g("B", parent: a.id), c = g("C")
        let tree = GroupTree(groups: [a, b, c])
        #expect(!tree.isEffectivelyEnabled(a.id))
        #expect(!tree.isEffectivelyEnabled(b.id))
        #expect(tree.isEffectivelyEnabled(c.id))
    }

    @Test func inheritedRuleAndInsertionUseNearestAncestor() {
        let outer = AppRule(mode: .allow, bundleIDs: ["x"])
        let inner = AppRule(mode: .deny, bundleIDs: ["y"])
        let a = g("A", rule: outer, insertion: .paste), b = g("B", parent: a.id, rule: inner), c = g("C", parent: b.id)
        let tree = GroupTree(groups: [a, b, c])
        #expect(tree.inheritedRule(forGroup: c.id)?.rule == inner)
        #expect(tree.inheritedRule(forGroup: c.id)?.from.id == b.id)
        #expect(tree.inheritedInsertion(forGroup: c.id)?.method == .paste)
        #expect(tree.inheritedRule(forGroup: nil) == nil)
    }

    // MARK: Moves

    @Test func cannotMoveGroupIntoItselfOrDescendant() {
        let a = g("A"), b = g("B", parent: a.id), c = g("C", parent: b.id)
        let tree = GroupTree(groups: [a, b, c])
        #expect(!tree.canMove(a.id, toParent: a.id))
        #expect(!tree.canMove(a.id, toParent: c.id))
        #expect(tree.reparent(a.id, toParent: b.id) == nil)
        #expect(tree.canMove(c.id, toParent: a.id))
        #expect(tree.canMove(c.id, toParent: nil))
        #expect(!tree.canMove(c.id, toParent: UUID()))
        #expect(!tree.canMove(UUID(), toParent: nil))
    }

    @Test func reparentAppendsAsLastChild() throws {
        let a = g("A", sort: 0), b = g("B", sort: 1), kid = g("Kid", parent: a.id, sort: 0)
        let tree = GroupTree(groups: [a, b, kid])
        let changed = try #require(tree.reparent(b.id, toParent: a.id))
        #expect(changed.count == 1)
        #expect(changed[0].id == b.id)
        #expect(changed[0].parentID == a.id)
        #expect(changed[0].sortIndex == 1)
    }

    @Test func reorderAmongSiblingsRenumbersOnlyWhatChanges() throws {
        let a = g("A", sort: 0), b = g("B", sort: 1), c = g("C", sort: 2)
        let tree = GroupTree(groups: [a, b, c])
        // Move C to the front.
        let changed = try #require(tree.reparent(c.id, toParent: nil, at: 0))
        let byID = Dictionary(uniqueKeysWithValues: changed.map { ($0.id, $0.sortIndex) })
        #expect(byID[c.id] == 0)
        #expect(byID[a.id] == 1)
        #expect(byID[b.id] == 2)
        // Applying the plan yields the expected order.
        var all = [a, b, c]
        for record in changed { all[all.firstIndex { $0.id == record.id }!] = record }
        #expect(GroupTree(groups: all).roots.map(\.group.name) == ["C", "A", "B"])
    }

    @Test func movingToTheSamePlaceChangesNothing() throws {
        let a = g("A", sort: 0), b = g("B", sort: 1)
        let tree = GroupTree(groups: [a, b])
        #expect(try #require(tree.reparent(b.id, toParent: nil, at: 1)).isEmpty)
    }

    @Test func indexIsClamped() throws {
        let a = g("A", sort: 0), b = g("B", sort: 1)
        let tree = GroupTree(groups: [a, b])
        let changed = try #require(tree.reparent(a.id, toParent: nil, at: 99))
        var all = [a, b]
        for record in changed { all[all.firstIndex { $0.id == record.id }!] = record }
        #expect(GroupTree(groups: all).roots.map(\.group.name) == ["B", "A"])
    }

    @Test func nextSortIndexAppends() {
        let a = g("A", sort: 3)
        let tree = GroupTree(groups: [a, g("K", parent: a.id, sort: 7)])
        #expect(tree.nextSortIndex(under: nil) == 4)
        #expect(tree.nextSortIndex(under: a.id) == 8)
        #expect(tree.nextSortIndex(under: UUID()) == 0)
    }
}
