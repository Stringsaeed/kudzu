import Foundation
import Testing
@testable import KudzuCore

@Suite("LibraryEditing")
struct LibraryEditingTests {
    @Test func savingKeepsZeroWidthSpaceButRemovesUnicodeWhitespaceOnlyTriggers() throws {
        let original = Snippet(triggers: ["\u{200B}", "\u{00A0}", "\n\t", ";ok"])
        var library = Library(snippets: [original])
        var renamed = original
        renamed.name = "Changed name only"
        let saved = try LibraryEditing.saveSnippet(renamed, in: &library)
        #expect(saved.triggers.count == 2)
        #expect(saved.triggers[0].utf8.elementsEqual("\u{200B}".utf8))
        #expect(saved.triggers[1] == ";ok")
        #expect(TriggerValidator.validate("\u{200B}", existing: []) == .success("\u{200B}"))
        #expect(TriggerValidator.validate("\u{00A0}", existing: []) == .failure(.whitespaceOnly))
    }

    private let t0 = Date(timeIntervalSince1970: 1_700_000_000)
    private let t1 = Date(timeIntervalSince1970: 1_700_000_500)
    private let t2 = Date(timeIntervalSince1970: 1_700_001_000)

    private func uuid(_ n: UInt8) -> UUID {
        UUID(uuid: (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, n))
    }

    @Test func createAndSaveSnippetPreserveCreatedAtStripBlankTriggersAndKeepFormatting() throws {
        var library = Library(snippets: [
            Snippet(id: uuid(1), name: "Existing", triggers: [";ex"], content: "existing", createdAt: t0, updatedAt: t0),
        ])

        let created = LibraryEditing.createSnippet(
            in: &library,
            draft: SnippetDraft(name: "Draft", triggers: [";dr"], content: "Hello world"),
            id: uuid(2),
            now: t1
        )
        #expect(created.id == uuid(2))
        #expect(created.createdAt == t1 && created.updatedAt == t1)
        #expect(library.snippets.map(\.id) == [uuid(2), uuid(1)])

        // Include a relative link span that JS editor validation permits so save_snippet storage
        // parity is verified (stored unchanged, not stripped on save).
        let formatting = Formatting(source: "Hello world", spans: [
            FormatSpan(start: 0, end: 5, style: .bold),
            FormatSpan(start: 6, end: 11, style: .link(url: "/docs/relative")),
        ])
        var edited = created
        edited.name = "Saved Draft"
        edited.triggers = [";dr", "   ", "\n\t", ";alt"]
        edited.formatting = formatting
        edited.createdAt = Date(timeIntervalSince1970: 99)

        let saved = try LibraryEditing.saveSnippet(edited, in: &library, now: t2)
        #expect(saved.id == uuid(2))
        #expect(saved.name == "Saved Draft")
        #expect(saved.createdAt == t1)
        #expect(saved.updatedAt == t2)
        #expect(saved.triggers == [";dr", ";alt"])
        #expect(saved.formatting == formatting)
        #expect(library.snippets[0] == saved)

        let missing = Snippet(id: uuid(99), name: "Ghost")
        #expect(throws: LibraryEditError.snippetNoLongerExists) {
            try LibraryEditing.saveSnippet(missing, in: &library, now: t2)
        }
        #expect(LibraryEditError.snippetNoLongerExists.errorDescription == "That snippet no longer exists.")
    }

    @Test func deleteAndIdempotentRestoreAvoidDuplicateSnippetIDs() {
        let a = Snippet(id: uuid(1), name: "A", triggers: [";a"], content: "alpha", createdAt: t0, updatedAt: t0)
        let b = Snippet(id: uuid(2), name: "B", triggers: [";b"], content: "bravo", createdAt: t0, updatedAt: t0)
        let c = Snippet(id: uuid(3), name: "C", triggers: [";c"], content: "charlie", createdAt: t0, updatedAt: t0)
        var library = Library(snippets: [a, b, c])

        LibraryEditing.deleteSnippets(ids: [b.id], in: &library)
        #expect(library.snippets.map(\.id) == [a.id, c.id])

        var staleA = a
        staleA.name = "Stale A"
        var duplicateB = b
        duplicateB.name = "Duplicate B"

        LibraryEditing.restoreSnippets([staleA, b, duplicateB], in: &library)
        #expect(library.snippets.map(\.id) == [a.id, c.id, b.id])
        #expect(library.snippets.first { $0.id == a.id }?.name == "A")
        #expect(library.snippets.first { $0.id == b.id }?.name == "B")
    }

    @Test func duplicateSnippetKeepsCopyNameAndFindsUnusedTriggersAfterSource() throws {
        let first = Snippet(
            id: uuid(1),
            name: "Signature",
            triggers: [";sig", ";work", ";chain", ";chain2"],
            content: "Best regards",
            createdAt: t0,
            updatedAt: t0
        )
        let existingCopy = Snippet(
            id: uuid(2),
            name: "signature copy",
            triggers: [";SIG2", ";sig3"],
            content: "Taken",
            createdAt: t0,
            updatedAt: t0
        )
        let untitled = Snippet(
            id: uuid(3),
            name: "",
            triggers: [";u"],
            content: "Untitled body",
            createdAt: t0,
            updatedAt: t0
        )
        var library = Library(snippets: [first, existingCopy, untitled])

        let copy = try LibraryEditing.duplicateSnippet(
            id: first.id,
            in: &library,
            newID: uuid(10),
            now: t1
        )
        #expect(copy.id == uuid(10))
        #expect(copy.name == "Signature copy")
        #expect(copy.triggers == [";sig4", ";work2", ";chain3", ";chain22"])
        #expect(copy.createdAt == t1 && copy.updatedAt == t1)
        #expect(library.snippets.map(\.id) == [first.id, uuid(10), existingCopy.id, untitled.id])

        let untitledCopy = try LibraryEditing.duplicateSnippet(
            id: untitled.id,
            in: &library,
            newID: uuid(11),
            now: t2
        )
        #expect(untitledCopy.name == "")
        #expect(untitledCopy.triggers == [";u2"])
        #expect(library.snippets.map(\.id) == [first.id, uuid(10), existingCopy.id, untitled.id, uuid(11)])

        #expect(throws: LibraryEditError.snippetNotFound) {
            try LibraryEditing.duplicateSnippet(id: uuid(99), in: &library, newID: uuid(12), now: t2)
        }
    }

    @Test func duplicateTriggerCollisionsUseExactLowercaseBytes() throws {
        let source = Snippet(id: uuid(1), triggers: [";e\u{301}", ";é"])
        let taken = Snippet(id: uuid(2), triggers: [";é2"])
        var library = Library(snippets: [source, taken])
        let copy = try LibraryEditing.duplicateSnippet(id: source.id, in: &library, newID: uuid(3), now: t1)
        #expect(copy.triggers.count == 2)
        #expect(copy.triggers[0].utf8.elementsEqual(";e\u{301}2".utf8))
        #expect(copy.triggers[1].utf8.elementsEqual(";é3".utf8))
    }

    @Test func moveSnippetsUpdatesTimestampsAndRefusesMissingGroup() throws {
        let group = Group(id: uuid(10), name: "Work", sortIndex: 0)
        let s1 = Snippet(id: uuid(1), name: "One", createdAt: t0, updatedAt: t0)
        let s2 = Snippet(id: uuid(2), name: "Two", groupID: group.id, createdAt: t0, updatedAt: t0)
        var library = Library(groups: [group], snippets: [s1, s2])

        #expect(throws: LibraryEditError.parentGroupNotFound) {
            try LibraryEditing.moveSnippets(ids: [s1.id], groupID: uuid(99), in: &library, now: t1)
        }
        #expect(library.snippets == [s1, s2])

        try LibraryEditing.moveSnippets(ids: [s1.id], groupID: group.id, in: &library, now: t1)
        #expect(library.snippets[0].groupID == group.id)
        #expect(library.snippets[0].updatedAt == t1)
        #expect(library.snippets[1].updatedAt == t0)

        try LibraryEditing.moveSnippets(ids: [s1.id, s2.id], groupID: nil, in: &library, now: t2)
        #expect(library.snippets.allSatisfy { $0.groupID == nil && $0.updatedAt == t2 })
    }

    @Test func createAndSaveGroupValidateHierarchyAndAssignPerParentSortIndex() throws {
        let rootA = Group(id: uuid(1), name: "Root A", parentID: nil, sortIndex: 0)
        let rootB = Group(id: uuid(2), name: "Root B", parentID: nil, sortIndex: 4)
        var library = Library(groups: [rootA, rootB])

        let child1 = try LibraryEditing.createGroup(name: "Child 1", parentID: rootA.id, in: &library, id: uuid(3))
        let child2 = try LibraryEditing.createGroup(name: "Child 2", parentID: rootA.id, in: &library, id: uuid(4))
        let rootC = try LibraryEditing.createGroup(name: "Root C", parentID: nil, in: &library, id: uuid(5))

        #expect(child1.sortIndex == 0)
        #expect(child2.sortIndex == 1)
        #expect(rootC.sortIndex == 5)

        #expect(throws: LibraryEditError.parentGroupNotFound) {
            try LibraryEditing.createGroup(name: "Orphan", parentID: uuid(99), in: &library, id: uuid(6))
        }

        // Cycle refusal: rootA -> child1 -> child2, moving rootA under child2 or itself must fail.
        var grandChild = child2
        grandChild.parentID = child1.id
        try LibraryEditing.saveGroup(grandChild, in: &library)

        var selfParent = rootA
        selfParent.parentID = rootA.id
        #expect(throws: LibraryEditError.groupCycle) {
            try LibraryEditing.saveGroup(selfParent, in: &library)
        }

        var descendantParent = rootA
        descendantParent.parentID = grandChild.id
        #expect(throws: LibraryEditError.groupCycle) {
            try LibraryEditing.saveGroup(descendantParent, in: &library)
        }
        #expect(LibraryEditError.groupCycle.errorDescription == "A group can't be moved inside itself.")

        var missingParent = rootA
        missingParent.parentID = uuid(99)
        #expect(throws: LibraryEditError.parentGroupNotFound) {
            try LibraryEditing.saveGroup(missingParent, in: &library)
        }

        #expect(throws: LibraryEditError.groupNotFound) {
            try LibraryEditing.saveGroup(Group(id: uuid(99), name: "Ghost"), in: &library)
        }
    }

    @Test func moveGroupMaintainsStableSiblingOrderingAndRefusesInvalidMoves() throws {
        // Bravo and Alpha share sortIndex 0 to verify deterministic name tie-breaking matching Groups.buildTree.
        let bravo = Group(id: uuid(1), name: "Bravo", parentID: nil, sortIndex: 0)
        let alpha = Group(id: uuid(2), name: "Alpha", parentID: nil, sortIndex: 0)
        let charlie = Group(id: uuid(3), name: "Charlie", parentID: nil, sortIndex: 5)
        let sub = Group(id: uuid(4), name: "Sub", parentID: alpha.id, sortIndex: 0)
        var library = Library(groups: [bravo, alpha, charlie, sub])

        // Reorder Charlie to index 1 at root: existing root siblings sort as [Alpha, Bravo],
        // so inserting Charlie at 1 yields [Alpha (0), Charlie (1), Bravo (2)].
        try LibraryEditing.moveGroup(id: charlie.id, parentID: nil, index: 1, in: &library)
        let rootOrder = library.groups
            .filter { $0.parentID == nil }
            .sorted { $0.sortIndex < $1.sortIndex }
        #expect(rootOrder.map(\.name) == ["Alpha", "Charlie", "Bravo"])
        #expect(rootOrder.map(\.sortIndex) == [0, 1, 2])

        // Move Charlie into Alpha at clamped index 99: Alpha's children become [Sub (0), Charlie (1)],
        // and remaining root siblings become [Alpha (0), Bravo (1)].
        try LibraryEditing.moveGroup(id: charlie.id, parentID: alpha.id, index: 99, in: &library)
        let alphaChildren = library.groups
            .filter { $0.parentID == alpha.id }
            .sorted { $0.sortIndex < $1.sortIndex }
        #expect(alphaChildren.map(\.name) == ["Sub", "Charlie"])
        #expect(alphaChildren.map(\.sortIndex) == [0, 1])

        let remainingRoot = library.groups
            .filter { $0.parentID == nil }
            .sorted { $0.sortIndex < $1.sortIndex }
        #expect(remainingRoot.map(\.name) == ["Alpha", "Bravo"])
        #expect(remainingRoot.map(\.sortIndex) == [0, 1])

        let snapshot = library
        #expect(throws: LibraryEditError.groupCycle) {
            try LibraryEditing.moveGroup(id: alpha.id, parentID: charlie.id, index: 0, in: &library)
        }
        #expect(throws: LibraryEditError.parentGroupNotFound) {
            try LibraryEditing.moveGroup(id: bravo.id, parentID: uuid(99), index: 0, in: &library)
        }
        #expect(throws: LibraryEditError.groupNotFound) {
            try LibraryEditing.moveGroup(id: uuid(99), parentID: nil, index: 0, in: &library)
        }
        #expect(library == snapshot)
    }

    @Test func deleteGroupRecursivelyRemovesDescendantsAndEitherKeepsOrDeletesSnippets() {
        let root = Group(id: uuid(1), name: "Root")
        let child = Group(id: uuid(2), name: "Child", parentID: root.id)
        let grand = Group(id: uuid(3), name: "Grand", parentID: child.id)
        let other = Group(id: uuid(4), name: "Other")

        let sRoot = Snippet(id: uuid(10), name: "In Root", groupID: root.id)
        let sChild = Snippet(id: uuid(11), name: "In Child", groupID: child.id)
        let sGrand = Snippet(id: uuid(12), name: "In Grand", groupID: grand.id)
        let sOther = Snippet(id: uuid(13), name: "In Other", groupID: other.id)
        let sLoose = Snippet(id: uuid(14), name: "Loose", groupID: nil)

        var keepLibrary = Library(
            groups: [root, child, grand, other],
            snippets: [sRoot, sChild, sGrand, sOther, sLoose]
        )
        let keepOutcome = LibraryEditing.deleteGroup(id: root.id, deleteSnippets: false, in: &keepLibrary)
        #expect(keepOutcome == GroupDeletionOutcome(
            deletedGroupIDs: [root.id, child.id, grand.id],
            deletedSnippetIDs: []
        ))
        #expect(keepLibrary.groups.map(\.id) == [other.id])
        #expect(keepLibrary.snippets.map(\.id) == [sRoot.id, sChild.id, sGrand.id, sOther.id, sLoose.id])
        #expect(keepLibrary.snippets.filter { $0.id != sOther.id }.allSatisfy { $0.groupID == nil })
        #expect(keepLibrary.snippets.first { $0.id == sOther.id }?.groupID == other.id)

        var removeLibrary = Library(
            groups: [root, child, grand, other],
            snippets: [sRoot, sChild, sGrand, sOther, sLoose]
        )
        let removeOutcome = LibraryEditing.deleteGroup(id: root.id, deleteSnippets: true, in: &removeLibrary)
        #expect(removeOutcome == GroupDeletionOutcome(
            deletedGroupIDs: [root.id, child.id, grand.id],
            deletedSnippetIDs: [sRoot.id, sChild.id, sGrand.id]
        ))
        #expect(removeLibrary.groups.map(\.id) == [other.id])
        #expect(removeLibrary.snippets.map(\.id) == [sOther.id, sLoose.id])
    }
}
