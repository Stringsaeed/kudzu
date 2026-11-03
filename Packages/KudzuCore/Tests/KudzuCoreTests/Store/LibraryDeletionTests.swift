import Foundation
import Testing
import SwiftData
@testable import KudzuCore

@MainActor
@Suite struct LibraryDeletionTests {
    @Test func deleteAndUndoRoundTripRestoresUsageAndGroups() throws {
        let store = try KudzuStore.inMemory()
        let parent = GroupRecord(name: "Parent")
        let doomed = GroupRecord(name: "Doomed", parentID: parent.id)
        let child = GroupRecord(name: "Child", parentID: doomed.id)
        let snippet = SnippetRecord(name: "s", triggers: [";s"], content: "S", groupID: child.id)
        for g in [parent, doomed, child] { try store.upsert(g) }
        try store.upsert(snippet)
        try store.recordUsage(snippetID: snippet.id, charsSaved: 4)

        let snapshot = try store.deleteGroupContents(rootID: doomed.id)
        #expect(try store.library().snippets.isEmpty)
        #expect(try store.library().groups.map(\.id) == [parent.id])

        let result = try store.restoreDeletion(snapshot)
        #expect(result.restoredSnippetIDs == [snippet.id])
        #expect(Set(result.restoredGroupIDs) == Set([doomed.id, child.id]))
        let library = try store.library()
        #expect(Set(library.groups.map(\.id)) == Set([parent.id, doomed.id, child.id]))
        #expect(library.snippets.map(\.id) == [snippet.id])
        #expect(library.usage.first?.snippetID == snippet.id)
        #expect(library.usage.first?.charsSaved == 4)
    }

    @Test func batchDeleteIsAllOrNothingWhenIDMissing() throws {
        let store = try KudzuStore.inMemory()
        let keep = SnippetRecord(name: "k", triggers: [";k"], content: "K")
        let drop = SnippetRecord(name: "d", triggers: [";d"], content: "D")
        try store.upsert(keep)
        try store.upsert(drop)
        let missing = UUID()
        #expect(throws: KudzuStoreError.snippetNotFound(missing)) {
            try store.deleteSnippets(ids: [keep.id, missing])
        }
        #expect(Set(try store.library().snippets.map(\.id)) == Set([keep.id, drop.id]))
    }

    @Test func moveContentsUpUndoRestoresHierarchy() throws {
        let store = try KudzuStore.inMemory()
        let parent = GroupRecord(name: "Parent")
        let doomed = GroupRecord(name: "Doomed", parentID: parent.id)
        let snippet = SnippetRecord(name: "s", triggers: [";s"], content: "S", groupID: doomed.id)
        try store.upsert(parent)
        try store.upsert(doomed)
        try store.upsert(snippet)

        let snapshot = try store.deleteGroupMovingContentsUp(id: doomed.id, moveChildrenTo: parent.id)
        let after = try store.library()
        #expect(after.groups.map(\.id) == [parent.id])
        #expect(after.snippets.first?.groupID == parent.id)

        _ = try store.restoreDeletion(snapshot)
        let restored = try store.library()
        #expect(Set(restored.groups.map(\.id)) == Set([parent.id, doomed.id]))
        #expect(restored.snippets.first?.groupID == doomed.id)
    }

    @Test func restoreRefusesReappearedSnippet() throws {
        let store = try KudzuStore.inMemory()
        let original = SnippetRecord(name: "a", triggers: [";a"], content: "A")
        try store.upsert(original)
        let snapshot = try store.deleteSnippets(ids: [original.id])
        var remote = original
        remote.content = "From sync"
        try store.upsert(remote)

        #expect(throws: (any Error).self) { try store.restoreDeletion(snapshot) }
        #expect(try store.library().snippets.first?.content == "From sync")
    }

    @Test func restoreRefusesEditedRelocation() throws {
        let store = try KudzuStore.inMemory()
        let parent = GroupRecord(name: "Parent")
        let doomed = GroupRecord(name: "Doomed", parentID: parent.id)
        var snippet = SnippetRecord(name: "s", triggers: [";s"], content: "S", groupID: doomed.id)
        try store.upsert(parent)
        try store.upsert(doomed)
        try store.upsert(snippet)

        let snapshot = try store.deleteGroupMovingContentsUp(id: doomed.id, moveChildrenTo: parent.id)
        snippet.groupID = parent.id
        snippet.content = "Edited after move"
        snippet.updatedAt = .now
        try store.upsert(snippet)

        #expect(throws: (any Error).self) { try store.restoreDeletion(snapshot) }
        #expect(try store.library().snippets.first?.content == "Edited after move")
        #expect(try store.library().groups.map(\.id) == [parent.id])
    }

    @Test func failedUndoLeavesSnapshotRecoverable() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "s", triggers: [";s"], content: "S")
        try store.upsert(snippet)
        let snapshot = try store.deleteSnippets(ids: [snippet.id])
        #expect(try store.library().snippets.isEmpty)

        // A missing parent must refuse the entire restore; the original snapshot remains retryable.
        var groupBound = snippet
        groupBound.groupID = UUID()
        let broken = LibraryDeletionSnapshot(
            kind: .snippets,
            deletedSnippets: [groupBound],
            deletedUsage: []
        )
        #expect(throws: (any Error).self) { try store.restoreDeletion(broken) }
        #expect(try store.library().snippets.isEmpty)

        let again = try store.restoreDeletion(snapshot)
        #expect(again.restoredSnippetIDs == [snippet.id])
        #expect(try store.library().snippets.map(\.id) == [snippet.id])
    }

    @Test func unrelatedRecordsSurviveDeleteAndUndo() throws {
        let store = try KudzuStore.inMemory()
        let keep = SnippetRecord(name: "keep", triggers: [";k"], content: "K")
        let drop = SnippetRecord(name: "drop", triggers: [";d"], content: "D")
        try store.upsert(keep)
        try store.upsert(drop)
        let snapshot = try store.deleteSnippets(ids: [drop.id])
        _ = try store.restoreDeletion(snapshot)
        #expect(try store.library().snippets.map(\.id) == [keep.id, drop.id])
    }
    @Test func reappearedIDRefusesEntireUndoAndCannotBeDeletedByRedo() throws {
        let store = try KudzuStore.inMemory()
        let a = SnippetRecord(name: "A", triggers: [";a"], content: "A")
        let b = SnippetRecord(name: "B", triggers: [";b"], content: "B")
        try store.upsert(a)
        try store.upsert(b)
        let snapshot = try store.deleteSnippets(ids: [a.id, b.id])
        var remote = a
        remote.content = "Concurrent record"
        try store.upsert(remote)
        #expect(throws: (any Error).self) { try store.restoreDeletion(snapshot) }
        #expect(try store.library().snippets == [remote])
        #expect(throws: (any Error).self) { try store.reapplyDeletion(snapshot) }
        #expect(try store.library().snippets == [remote])
    }

    @Test func redoRefusesEditedRestoredSnippet() throws {
        let store = try KudzuStore.inMemory()
        var snippet = SnippetRecord(name: "S", triggers: [";s"], content: "Original")
        try store.upsert(snippet)
        let snapshot = try store.deleteSnippets(ids: [snippet.id])
        try store.restoreDeletion(snapshot)
        snippet.content = "Remote edit"
        try store.upsert(snippet)
        #expect(throws: (any Error).self) { try store.reapplyDeletion(snapshot) }
        #expect(try store.library().snippets == [snippet])
    }

    @Test func redoRefusesNewDescendantsAndUsage() throws {
        let store = try KudzuStore.inMemory()
        let group = GroupRecord(name: "G")
        let original = SnippetRecord(name: "S", triggers: [";s"], content: "S", groupID: group.id)
        try store.upsert(group)
        try store.upsert(original)
        let snapshot = try store.deleteGroupContents(rootID: group.id)
        try store.restoreDeletion(snapshot)
        let newChild = GroupRecord(name: "New child", parentID: group.id)
        try store.upsert(newChild)
        let before = try store.library()
        #expect(throws: (any Error).self) { try store.reapplyDeletion(snapshot) }
        try expectLibrary(store, matches: before)
        try store.deleteGroup(id: newChild.id, moveChildrenTo: nil)
        try store.recordUsage(snippetID: original.id, charsSaved: 7)
        let used = try store.library()
        #expect(throws: (any Error).self) { try store.reapplyDeletion(snapshot) }
        try expectLibrary(store, matches: used)
    }

    @Test func moveToDifferentParentRoundTripsExactly() throws {
        let store = try KudzuStore.inMemory()
        let parent = GroupRecord(name: "Original parent")
        let target = GroupRecord(name: "Other parent")
        let group = GroupRecord(name: "G", parentID: parent.id)
        let child = GroupRecord(name: "C", parentID: group.id)
        let snippet = SnippetRecord(name: "S", triggers: [";s"], content: "S", groupID: group.id)
        for g in [parent, target, group, child] { try store.upsert(g) }
        try store.upsert(snippet)
        let before = try store.library()
        let snapshot = try store.deleteGroupMovingContentsUp(id: group.id, moveChildrenTo: target.id)
        let after = try store.library()
        try store.restoreDeletion(snapshot)
        try expectLibrary(store, matches: before)
        try store.reapplyDeletion(snapshot)
        try expectLibrary(store, matches: after)
    }

    @Test func undoRefusesRemotelyDeletedMovedChild() throws {
        let store = try KudzuStore.inMemory()
        let group = GroupRecord(name: "G")
        let child = GroupRecord(name: "C", parentID: group.id)
        try store.upsert(group)
        try store.upsert(child)
        let snapshot = try store.deleteGroupMovingContentsUp(id: group.id, moveChildrenTo: nil)
        try store.deleteGroup(id: child.id, moveChildrenTo: nil)
        #expect(throws: (any Error).self) { try store.restoreDeletion(snapshot) }
        #expect(try store.library().groups.isEmpty)
    }

    @Test func undoRefusesConflictingGroupWithoutReparentingChildren() throws {
        let store = try KudzuStore.inMemory()
        var group = GroupRecord(name: "G")
        let snippet = SnippetRecord(name: "S", triggers: [";s"], content: "S", groupID: group.id)
        try store.upsert(group)
        try store.upsert(snippet)
        let snapshot = try store.deleteGroupMovingContentsUp(id: group.id, moveChildrenTo: nil)
        group.name = "Concurrent group"
        try store.upsert(group)
        let before = try store.library()
        #expect(throws: (any Error).self) { try store.restoreDeletion(snapshot) }
        try expectLibrary(store, matches: before)
    }

    @Test func undoRefusesMissingRootParentWithoutPartialTopology() throws {
        let store = try KudzuStore.inMemory()
        let parent = GroupRecord(name: "P")
        let group = GroupRecord(name: "G", parentID: parent.id)
        let child = GroupRecord(name: "C", parentID: group.id)
        for g in [parent, group, child] { try store.upsert(g) }
        let snapshot = try store.deleteGroupMovingContentsUp(id: group.id, moveChildrenTo: nil)
        try store.deleteGroup(id: parent.id, moveChildrenTo: nil)
        let before = try store.library()
        #expect(throws: (any Error).self) { try store.restoreDeletion(snapshot) }
        try expectLibrary(store, matches: before)
    }

    @Test func failedRestoreRollsBackEarlierInsertionsWithoutCallerRollback() throws {
        let store = try KudzuStore.inMemory()
        let valid = GroupRecord(name: "Valid")
        var cycle = GroupRecord(name: "Invalid")
        cycle.parentID = cycle.id
        let snapshot = LibraryDeletionSnapshot(kind: .groupDeleteContents(groupID: valid.id), deletedGroups: [valid, cycle])
        #expect(throws: (any Error).self) { try store.restoreDeletion(snapshot) }
        #expect(try store.library().groups.isEmpty)
        #expect(!store.context.hasChanges)
        let unrelated = SnippetRecord(name: "Unrelated", triggers: [], content: "Keep")
        try store.upsert(unrelated)
        #expect(try store.library().groups.isEmpty)
    }

    @Test func undoCommitsGroupsSnippetsAndUsageTogether() throws {
        let store = try KudzuStore.inMemory()
        let group = GroupRecord(name: "G")
        let snippet = SnippetRecord(name: "S", triggers: [], content: "S", groupID: group.id)
        try store.upsert(group)
        try store.upsert(snippet)
        try store.recordUsage(snippetID: snippet.id, charsSaved: 5)
        let original = try store.library()
        let snapshot = try store.deleteGroupContents(rootID: group.id)
        var commits: [KudzuLibrary] = []
        store.commitChanges = { context in
            commits.append(try store.library())
            try context.save()
        }
        try store.restoreDeletion(snapshot)
        #expect(commits.count == 1)
        #expect(commits.first?.groups == original.groups)
        #expect(commits.first?.snippets == original.snippets)
        #expect(commits.first?.usage == original.usage)
    }

    @Test(arguments: ["snippets", "group", "move", "restore", "redo", "restoreMove", "redoMove"])
    func saveFailureRollsBackAtEveryDeletionEntryPoint(operation: String) throws {
        enum Failure: Error { case diskFull }
        let store = try KudzuStore.inMemory()
        let group = GroupRecord(name: "G")
        let child = GroupRecord(name: "C", parentID: group.id)
        let snippet = SnippetRecord(name: "S", triggers: [], content: "S", groupID: group.id)
        for g in [group, child] { try store.upsert(g) }
        try store.upsert(snippet)
        try store.recordUsage(snippetID: snippet.id, charsSaved: 5)
        var snapshot: LibraryDeletionSnapshot?
        if operation.hasPrefix("restore") || operation.hasPrefix("redo") {
            snapshot = operation.hasSuffix("Move")
                ? try store.deleteGroupMovingContentsUp(id: group.id, moveChildrenTo: nil)
                : try store.deleteGroupContents(rootID: group.id)
            if operation.hasPrefix("redo") { try store.restoreDeletion(snapshot!) }
        }
        let before = try store.library()
        store.commitChanges = { _ in throw Failure.diskFull }
        #expect(throws: Failure.diskFull) {
            switch operation {
            case "snippets": try store.deleteSnippet(id: snippet.id)
            case "group": try store.deleteGroupContents(rootID: group.id)
            case "move": try store.deleteGroup(id: group.id, moveChildrenTo: nil)
            case "restore", "restoreMove": try store.restoreDeletion(snapshot!)
            default: try store.reapplyDeletion(snapshot!)
            }
        }
        try expectLibrary(store, matches: before)
        #expect(!store.context.hasChanges)
        store.commitChanges = { try $0.save() }
        try store.upsert(SnippetRecord(name: "Unrelated", triggers: [], content: "Later save"))
        #expect(try store.library().groups == before.groups)
        #expect(try store.library().snippets.filter { $0.id == snippet.id } == before.snippets)
        #expect(try store.library().usage == before.usage)
    }

    @Test func deletionUndoPreservesDedupeAliasesForLateUsage() throws {
        let store = try KudzuStore.inMemory()
        let a = SnippetRecord(name: "S", triggers: [";s"], content: "Same")
        let b = SnippetRecord(name: "S", triggers: [";s"], content: "Same")
        try store.upsert(a)
        try store.upsert(b)
        try store.dedupe()
        let survivor = try #require(store.library().snippets.first)
        let oldID = survivor.id == a.id ? b.id : a.id
        let snapshot = try store.deleteSnippets(ids: [survivor.id])
        try store.restoreDeletion(snapshot)
        try store.recordUsage(snippetID: oldID, charsSaved: 6)
        try store.dedupe()
        let usage = try #require(store.library().usage.first)
        #expect(usage.snippetID == survivor.id)
        #expect(usage.charsSaved == 6)
    }

    @Test func redoRefusesContentEditedByAnIndependentContext() throws {
        let store = try KudzuStore.inMemory()
        let snippet = SnippetRecord(name: "S", triggers: [], content: "Original")
        try store.upsert(snippet)
        let snapshot = try store.deleteSnippets(ids: [snippet.id])
        try store.restoreDeletion(snapshot)
        let writer = ModelContext(store.container)
        let model = try #require(writer.fetch(FetchDescriptor<SnippetModel>()).first)
        model.content = "Independent edit"
        try writer.save()
        #expect(throws: (any Error).self) { try store.reapplyDeletion(snapshot) }
        let reader = ModelContext(store.container)
        #expect(try reader.fetch(FetchDescriptor<SnippetModel>()).first?.content == "Independent edit")
    }

}

@MainActor private func expectLibrary(_ store: KudzuStore, matches expected: KudzuLibrary) throws {
    let actual = try store.library()
    #expect(actual.snippets == expected.snippets)
    #expect(actual.groups == expected.groups)
    #expect(actual.usage == expected.usage)
}
