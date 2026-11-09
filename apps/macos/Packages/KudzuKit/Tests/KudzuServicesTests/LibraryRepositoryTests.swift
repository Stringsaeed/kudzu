import Foundation
import KudzuCore
import SwiftData
import Testing
@testable import KudzuServices

// Disk-backed tests of the SwiftData prototype. Every test opens a real `ModelContainer` on a
// file under the temporary directory with `cloudKitDatabase: .none`, so nothing here says
// anything about live CloudKit; see docs/SWIFTDATA_PROTOTYPE.md.

@Suite("SwiftData library repository", .serialized)
struct LibraryRepositoryTests {
    @Test func applyingUnchangedValuesLeavesTheContextClean() async throws {
        let dir = scratchStoreDir("unchanged-rows")
        defer { try? FileManager.default.removeItem(at: dir) }
        let fixture = try frozenLibraryFixture()
        let repo = try await openLocalRepository(at: dir)
        try await repo.importLibrary(fixture)

        let context = ModelContext(repo.modelContainer)
        context.autosaveEnabled = false
        let snippets = try context.fetch(FetchDescriptor<PersistedSnippet>())
        let groups = try context.fetch(FetchDescriptor<PersistedGroup>())
        let usage = try context.fetch(FetchDescriptor<PersistedUsage>())
        #expect(!context.hasChanges)

        for (position, value) in fixture.snippets.enumerated() {
            let row = try #require(snippets.first { $0.id == value.id })
            try LibraryRowCodec.apply(value, listOrder: position, to: row)
        }
        for (position, value) in fixture.groups.enumerated() {
            let row = try #require(groups.first { $0.id == value.id })
            try LibraryRowCodec.apply(value, listOrder: position, to: row)
        }
        for (position, value) in fixture.usage.enumerated() {
            let row = try #require(usage.first {
                $0.snippetID == value.snippetID && $0.deviceIDUTF8 == Data(value.deviceID.utf8)
            })
            LibraryRowCodec.apply(value, listOrder: position, to: row)
        }
        #expect(!context.hasChanges)
        #expect(context.changedModelsArray.isEmpty)

        // The same codec must still stage a real edit.
        var changed = try #require(fixture.snippets.first)
        changed.content += " edited"
        let row = try #require(snippets.first { $0.id == changed.id })
        try LibraryRowCodec.apply(changed, listOrder: 0, to: row)
        #expect(context.hasChanges)
        #expect(context.changedModelsArray.count == 1)
        context.rollback()
    }

    // MARK: Fidelity

    @Test func frozenFixtureRoundTripsThroughDiskAndReopen() async throws {
        let dir = scratchStoreDir("fixture")
        defer { try? FileManager.default.removeItem(at: dir) }
        let fixture = try frozenLibraryFixture()
        #expect(fixture.snippets.count == 3 && fixture.groups.count == 3 && fixture.usage.count == 3)

        var repo: LibraryRepository? = try await openLocalRepository(at: dir)
        try await repo!.importLibrary(fixture)
        let counts = try await repo!.rowCounts()
        #expect(counts.snippets == 3 && counts.groups == 3 && counts.usage == 3)
        #expect(try await repo!.loadLibrary() == fixture)
        #expect(repo!.snapshot == fixture)

        // Literal spot checks, not only `==`.
        let loaded = try await repo!.loadLibrary()
        let everything = try #require(loaded.snippets.first { $0.id == UUID(uuidString: "a1b2c3d4-e5f6-4a7b-8c9d-00000000000a") })
        #expect(everything.triggers == [";all", ";ALL2"])
        #expect(everything.fireMode == .wholeWord && everything.caseMode == .exact && everything.insertion == .type)
        #expect(everything.rule == AppRule(mode: .deny, bundleIDs: ["com.tinyspeck.slackmacgap", "com.apple.Terminal"], insertion: .paste))
        #expect(everything.formatting?.spans.map(\.style.kind) == ["bold", "italic", "underline", "link", "image"])
        #expect(everything.formatting?.source == everything.content)
        if case let .image(dataUrl, width)? = everything.formatting?.spans.last?.style {
            #expect(dataUrl.hasPrefix("data:image/png;base64,iVBORw0KGgo"))
            #expect(width >= 1)
        } else {
            Issue.record("image span missing")
        }
        #expect(KudzuDate.formatWholeSeconds(everything.createdAt) == "2026-09-29T09:30:15Z")
        #expect(KudzuDate.formatWholeSeconds(everything.updatedAt) == "2026-11-08T09:30:15Z")
        #expect(everything.groupID == UUID(uuidString: "a1b2c3d4-e5f6-4a7b-8c9d-000000000002"))
        let archive = try #require(loaded.groups.first { $0.name == "Archive — old" })
        #expect(archive.parentID == UUID(uuidString: "a1b2c3d4-e5f6-4a7b-8c9d-000000000002") && archive.sortIndex == 2 && archive.isEnabled == false)
        #expect(archive.rule == AppRule(mode: .deny, bundleIDs: [], insertion: .type))
        #expect(loaded.usage[1] == UsageRecord(snippetID: everything.id, deviceID: "macbook", count: 3, charsSaved: -2, lastUsedAt: nil))
        #expect(loaded.usage[2].deviceID == "" && loaded.usage[2].count == 0)

        // Reopen from disk with a new container and a new actor.
        repo = nil
        let reopened = try await openLocalRepository(at: dir)
        #expect(reopened.snapshot == fixture)
        #expect(try await reopened.loadLibrary() == fixture)
        #expect(FileManager.default.fileExists(atPath: dir.appendingPathComponent("Library.store").path))
    }

    @Test func edgeCasesKeepBytesOrderAncestryAndDates() async throws {
        let dir = scratchStoreDir("edge")
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = EdgeLibrary.library

        var repo: LibraryRepository? = try await openLocalRepository(at: dir)
        try await repo!.importLibrary(source)
        repo = nil
        let reopened = try await openLocalRepository(at: dir)
        let loaded = reopened.snapshot
        #expect(loaded == source)

        // Array order survives although groups are not in sortIndex order and snippets not in date order.
        #expect(loaded.groups.map(\.id) == [uuid("3"), uuid("1"), uuid("4"), uuid("2")])
        #expect(loaded.snippets.map(\.id) == [uuid("c"), uuid("a"), uuid("b")])
        #expect(loaded.usage.map(\.deviceID) == ["mac-studio", "macbook", "", "mac-studio"])

        // Byte-level Unicode: `String ==` treats NFC and NFD as equal, so compare UTF-8.
        let rich = try #require(loaded.snippets.first { $0.id == uuid("a") })
        #expect(utf8(rich.name) == utf8(EdgeLibrary.nfd))
        #expect(utf8(rich.name) != utf8(EdgeLibrary.nfc))
        #expect(rich.triggers.map(utf8) == [";" + EdgeLibrary.nfc, ";" + EdgeLibrary.nfd, "", "  ", "x"].map(utf8))
        #expect(utf8(rich.content) == utf8(EdgeLibrary.rich.content))
        #expect(rich.content.utf8.contains(0))      // the NUL made it
        #expect(utf8(loaded.groups[3].name) == utf8(EdgeLibrary.nfd))
        #expect(utf8(loaded.groups[0].name) == utf8(EdgeLibrary.nfc))

        // Formatting: every span, the ~240 KB image, the unicode link.
        let spans = try #require(rich.formatting?.spans)
        #expect(spans.count == 6)
        #expect(spans == EdgeLibrary.rich.formatting?.spans)
        if case let .image(dataUrl, width) = spans[5].style {
            #expect(dataUrl.utf8.count == EdgeLibrary.bigImage.utf8.count && width == 512)
        } else { Issue.record("big image span lost") }
        if case let .link(url) = spans[3].style { #expect(url == "https://例え.jp/path?q=a&b=c#frag") } else { Issue.record("link lost") }
        #expect(utf8(rich.formatting!.source) == utf8(rich.content))
        let empty = try #require(loaded.snippets.first { $0.id == uuid("b") })
        #expect(empty.formatting == Formatting(source: "", spans: []))   // empty spans ≠ nil formatting
        let plain = try #require(loaded.snippets.first { $0.id == uuid("c") })
        #expect(plain.formatting == nil && plain.groupID == uuid("9"))     // dangling group id kept as data

        // Modes, rules, flags.
        #expect(rich.fireMode == .wholeWord && rich.caseMode == .exact && rich.insertion == .type && rich.isFavorite && !rich.isEnabled)
        #expect(rich.rule == AppRule(mode: .deny, bundleIDs: ["com.tinyspeck.slackmacgap"], insertion: .paste))
        #expect(loaded.groups[1].rule == AppRule(mode: .allow, bundleIDs: ["com.apple.mail", "com.apple.Notes"], insertion: .type))
        #expect(loaded.groups[0].rule == AppRule(mode: .deny, bundleIDs: []))

        // Ancestry and sort indexes, including two roots with the same name and index.
        #expect(loaded.groups[3].parentID == uuid("1") && loaded.groups[0].parentID == uuid("2"))
        #expect(loaded.groups.map(\.sortIndex) == [0, 5, 5, 0])
        #expect(Groups.descendantIds(loaded.groups, id: uuid("1")) == [uuid("1"), uuid("2"), uuid("3")])

        // Dates: exact Double round trip, including the sub-second one.
        #expect(rich.createdAt == t1 && rich.createdAt.timeIntervalSince1970 == 1_762_600_000.123456)
        #expect(rich.updatedAt == t0 && empty.updatedAt == t2)
        #expect(loaded.usage[0].lastUsedAt == t1 && loaded.usage[1].lastUsedAt == nil)
        #expect(loaded.usage[1].charsSaved == -2 && loaded.usage[3].snippetID == uuid("f"))
    }

    @Test func byteOnlyEditsAndDeviceIdentitiesSurviveReopen() async throws {
        let dir = scratchStoreDir("byte-edit")
        defer { try? FileManager.default.removeItem(at: dir) }
        var repo: LibraryRepository? = try await openLocalRepository(at: dir)
        let snippet = Snippet(id: uuid("a"), name: EdgeLibrary.nfc, content: EdgeLibrary.nfc)
        try await repo!.importLibrary(Library(groups: [Group(id: uuid("1"), name: EdgeLibrary.nfc)], snippets: [snippet]))
        try await repo!.edit {
            $0.snippets[0].name = EdgeLibrary.nfd + "\u{0}tail"
            $0.snippets[0].content = EdgeLibrary.nfd
            $0.groups[0].name = EdgeLibrary.nfd + "\u{0}tail"
        }
        try await repo!.recordUsage(snippetID: snippet.id, deviceID: EdgeLibrary.nfc, charsSaved: 1, now: t1)
        try await repo!.recordUsage(snippetID: snippet.id, deviceID: EdgeLibrary.nfd, charsSaved: 2, now: t1)
        try await repo!.recordUsage(snippetID: snippet.id, deviceID: "mac\u{0}tail", charsSaved: 3, now: t1)
        repo = nil
        let reopened = try await openLocalRepository(at: dir)
        #expect(utf8(reopened.snapshot.snippets[0].content) == utf8(EdgeLibrary.nfd))
        #expect(utf8(reopened.snapshot.snippets[0].name) == utf8(EdgeLibrary.nfd + "\u{0}tail"))
        #expect(utf8(reopened.snapshot.groups[0].name) == utf8(EdgeLibrary.nfd + "\u{0}tail"))
        #expect(reopened.snapshot.usage.map { utf8($0.deviceID) } == [EdgeLibrary.nfc, EdgeLibrary.nfd, "mac\u{0}tail"].map(utf8))
        #expect(reopened.snapshot.usage.map(\.charsSaved) == [1, 2, 3])
    }

    @Test func invalidImportsLeaveCommittedDiskUntouched() async throws {
        let dir = scratchStoreDir("invalid-import")
        defer { try? FileManager.default.removeItem(at: dir) }
        var repo: LibraryRepository? = try await openLocalRepository(at: dir)
        let fixture = try frozenLibraryFixture()
        try await repo!.importLibrary(fixture)
        var invalid = fixture
        invalid.snippets.append(fixture.snippets[0])
        let duplicate = invalid
        await #expect(throws: LibraryRepositoryError.duplicateIdentity(entity: "Library.snippets", keys: [fixture.snippets[0].id.lowercased])) {
            try await repo!.importLibrary(duplicate)
        }
        invalid = fixture
        invalid.groups.append(fixture.groups[0])
        let groups = invalid
        await #expect(throws: LibraryRepositoryError.duplicateIdentity(entity: "Library.groups", keys: [fixture.groups[0].id.lowercased])) {
            try await repo!.importLibrary(groups)
        }
        invalid = fixture
        invalid.formatVersion = 999
        let future = invalid
        await #expect(throws: LibraryRepositoryError.unsupportedFormatVersion(999)) {
            try await repo!.importLibrary(future)
        }
        repo = nil
        let reopened = try await openLocalRepository(at: dir)
        #expect(reopened.snapshot == fixture)
        let counts = try await reopened.rowCounts()
        #expect(counts.snippets == 3 && counts.groups == 3 && counts.usage == 3)
    }

    // MARK: Editing through LibraryEditing

    @Test func libraryEditingMutationsPersistAndReopenIdentically() async throws {
        let dir = scratchStoreDir("editing")
        defer { try? FileManager.default.removeItem(at: dir) }
        let fixture = try frozenLibraryFixture()
        var repo: LibraryRepository? = try await openLocalRepository(at: dir)
        try await repo!.importLibrary(fixture)

        // The same calls, with the same ids and clock, on a plain value: the expected outcome.
        var expected = fixture
        let workID = UUID(uuidString: "a1b2c3d4-e5f6-4a7b-8c9d-000000000001")!
        let emailID = UUID(uuidString: "a1b2c3d4-e5f6-4a7b-8c9d-000000000002")!
        let everythingID = UUID(uuidString: "a1b2c3d4-e5f6-4a7b-8c9d-00000000000a")!
        let draft = SnippetDraft(name: "New", triggers: [";n"], content: "n {{cursor}}", formatting: Formatting(source: "n {{cursor}}", spans: [FormatSpan(start: 0, end: 1, style: .bold)]), groupID: workID, fireMode: .wholeWord, caseMode: .exact, insertion: .paste, rule: AppRule(mode: .allow, bundleIDs: ["com.apple.Notes"]), isFavorite: true, isEnabled: true)

        // 1. create: inserted at the front, stamped `t0`.
        let created = try await repo!.edit { LibraryEditing.createSnippet(in: &$0, draft: draft, id: uuid("10"), now: t0) }
        LibraryEditing.createSnippet(in: &expected, draft: draft, id: uuid("10"), now: t0)
        #expect(created.createdAt == t0 && created.updatedAt == t0 && created.formatting == draft.formatting)
        #expect(repo!.snapshot.snippets.first?.id == uuid("10"))
        #expect(repo!.snapshot == expected)

        // 2. save: createdAt kept, updatedAt = now, blank triggers stripped, formatting as given.
        let edited: Snippet = {
            var e = created
            e.name = "Renamed"
            e.triggers = [";n", "", "   ", ";nn"]
            e.createdAt = .distantPast   // must be ignored
            e.formatting = nil
            return e
        }()
        let saved = try await repo!.edit { try LibraryEditing.saveSnippet(edited, in: &$0, now: t2) }
        _ = try LibraryEditing.saveSnippet(edited, in: &expected, now: t2)
        #expect(saved.triggers == [";n", ";nn"] && saved.createdAt == t0 && saved.updatedAt == t2 && saved.formatting == nil)
        #expect(repo!.snapshot == expected)

        // 3. duplicate: lands at index + 1 with numbered triggers.
        let copy = try await repo!.edit { try LibraryEditing.duplicateSnippet(id: everythingID, in: &$0, newID: uuid("11"), now: t2) }
        _ = try LibraryEditing.duplicateSnippet(id: everythingID, in: &expected, newID: uuid("11"), now: t2)
        #expect(copy.name == "Everything set copy" && copy.triggers == [";all3", ";ALL22"])
        #expect(copy.formatting == fixture.snippets[0].formatting && copy.rule == fixture.snippets[0].rule)
        let afterDup = repo!.snapshot
        #expect(afterDup.snippets.firstIndex { $0.id == uuid("11") } == afterDup.snippets.firstIndex { $0.id == everythingID }! + 1)
        #expect(afterDup == expected)

        // 4. move snippets to the Email group.
        try await repo!.edit { try LibraryEditing.moveSnippets(ids: [uuid("10"), uuid("11")], groupID: emailID, in: &$0, now: t2) }
        try LibraryEditing.moveSnippets(ids: [uuid("10"), uuid("11")], groupID: emailID, in: &expected, now: t2)
        #expect(repo!.snapshot.snippets.filter { $0.groupID == emailID }.count == 3)
        #expect(repo!.snapshot == expected)

        // 5. groups: create under Work (sortIndex after Email), rename, move to the root at index 0, delete Email keeping snippets.
        let group = try await repo!.edit { try LibraryEditing.createGroup(name: "Personal", parentID: workID, in: &$0, id: uuid("20")) }
        _ = try LibraryEditing.createGroup(name: "Personal", parentID: workID, in: &expected, id: uuid("20"))
        #expect(group.sortIndex == 1 && group.parentID == workID)
        let renamed: Group = {
            var r = group
            r.name = "Personal ✓"
            r.rule = AppRule(mode: .deny, bundleIDs: ["com.apple.Terminal"], insertion: .type)
            return r
        }()
        try await repo!.edit { try LibraryEditing.saveGroup(renamed, in: &$0) }
        _ = try LibraryEditing.saveGroup(renamed, in: &expected)
        try await repo!.edit { try LibraryEditing.moveGroup(id: uuid("20"), parentID: nil, index: 0, in: &$0) }
        try LibraryEditing.moveGroup(id: uuid("20"), parentID: nil, index: 0, in: &expected)
        #expect(repo!.snapshot.groups.first { $0.id == uuid("20") }?.parentID == nil)
        #expect(repo!.snapshot.groups.first { $0.id == uuid("20") }?.sortIndex == 0)
        #expect(repo!.snapshot.groups.first { $0.id == workID }?.sortIndex == 1)
        #expect(repo!.snapshot == expected)

        let outcome = try await repo!.edit { LibraryEditing.deleteGroup(id: emailID, deleteSnippets: false, in: &$0) }
        let expectedOutcome = LibraryEditing.deleteGroup(id: emailID, deleteSnippets: false, in: &expected)
        #expect(outcome == expectedOutcome)
        #expect(outcome.deletedGroupIDs == [emailID, UUID(uuidString: "a1b2c3d4-e5f6-4a7b-8c9d-000000000003")!] && outcome.deletedSnippetIDs.isEmpty)
        #expect(repo!.snapshot.snippets.allSatisfy { $0.groupID != emailID })
        #expect(repo!.snapshot == expected)

        // Rows match the value: nothing leaked.
        let counts = try await repo!.rowCounts()
        #expect(counts.snippets == expected.snippets.count && counts.groups == expected.groups.count && counts.usage == expected.usage.count)
        #expect(counts.snippets == 5 && counts.groups == 2)

        // Reopen: the disk agrees with every step above.
        repo = nil
        let reopened = try await openLocalRepository(at: dir)
        #expect(reopened.snapshot == expected)
    }

    @Test func deleteThenUndoRestoreKeepsEveryField() async throws {
        let dir = scratchStoreDir("undo")
        defer { try? FileManager.default.removeItem(at: dir) }
        let fixture = try frozenLibraryFixture()
        let everything = fixture.snippets[0]
        var repo: LibraryRepository? = try await openLocalRepository(at: dir)
        try await repo!.importLibrary(fixture)

        try await repo!.edit { LibraryEditing.deleteSnippets(ids: [everything.id], in: &$0) }
        #expect(repo!.snapshot.snippets.map(\.id) == Array(fixture.snippets.map(\.id).dropFirst()))
        #expect(try await repo!.rowCounts().snippets == 2)
        #expect(try await repo!.rowCounts().usage == 3)   // usage rows are data; deletion of the snippet does not erase them

        repo = nil
        repo = try await openLocalRepository(at: dir)
        #expect(repo!.snapshot.snippets.count == 2)

        // Undo: the UI passes back the deleted values.
        try await repo!.edit { LibraryEditing.restoreSnippets([everything], in: &$0) }
        let restored = try #require(repo!.snapshot.snippets.last)
        #expect(restored == everything)                                   // every field, dates included
        #expect(restored.formatting == everything.formatting && restored.rule == everything.rule)
        #expect(repo!.snapshot.snippets.map(\.id) == [fixture.snippets[1].id, fixture.snippets[2].id, everything.id])
        #expect(try await repo!.rowCounts().snippets == 3)

        // A second restore of the same snippet is ignored (LibraryEditing rule) and adds no row.
        try await repo!.edit { LibraryEditing.restoreSnippets([everything], in: &$0) }
        #expect(try await repo!.rowCounts().snippets == 3)

        repo = nil
        let reopened = try await openLocalRepository(at: dir)
        #expect(reopened.snapshot.snippets.last == everything)
    }

    @Test func recordUsageKeepsOneRowPerDevice() async throws {
        let dir = scratchStoreDir("usage")
        defer { try? FileManager.default.removeItem(at: dir) }
        let fixture = try frozenLibraryFixture()
        let everything = fixture.snippets[0]
        var repo: LibraryRepository? = try await openLocalRepository(at: dir)
        try await repo!.importLibrary(fixture)

        try await repo!.recordUsage(snippetID: everything.id, deviceID: "mac-studio", charsSaved: 10, now: t2)
        try await repo!.recordUsage(snippetID: everything.id, deviceID: "new-mac", charsSaved: -5, now: t2)
        try await repo!.recordUsage(snippetID: everything.id, deviceID: "new-mac", charsSaved: 7, now: t2)

        let usage = repo!.snapshot.usage
        #expect(usage.count == 4)
        #expect(usage[0] == UsageRecord(snippetID: everything.id, deviceID: "mac-studio", count: 129, charsSaved: 4106, lastUsedAt: t2))
        #expect(usage[1] == UsageRecord(snippetID: everything.id, deviceID: "macbook", count: 3, charsSaved: -2, lastUsedAt: nil))  // another device's row untouched
        #expect(usage[3] == UsageRecord(snippetID: everything.id, deviceID: "new-mac", count: 2, charsSaved: 7, lastUsedAt: t2))
        #expect(try await repo!.rowCounts().usage == 4)

        repo = nil
        let reopened = try await openLocalRepository(at: dir)
        #expect(reopened.snapshot.usage == usage)
    }

    // MARK: Import idempotence and interruption

    @Test func repeatedImportAddsNoRows() async throws {
        let dir = scratchStoreDir("reimport")
        defer { try? FileManager.default.removeItem(at: dir) }
        let fixture = try frozenLibraryFixture()
        var repo: LibraryRepository? = try await openLocalRepository(at: dir)

        for _ in 0..<3 {
            try await repo!.importLibrary(fixture)
            let counts = try await repo!.rowCounts()
            #expect(counts.snippets == 3 && counts.groups == 3 && counts.usage == 3)
        }
        repo = nil
        repo = try await openLocalRepository(at: dir)
        try await repo!.importLibrary(fixture)          // after a reopen too
        var counts = try await repo!.rowCounts()
        #expect(counts.snippets == 3 && counts.groups == 3 && counts.usage == 3)
        #expect(repo!.snapshot == fixture)

        // A smaller import removes the absent rows; the original import brings them back once.
        var smaller = fixture
        smaller.snippets.removeFirst()
        smaller.groups.removeLast()
        smaller.usage.removeAll { $0.deviceID == "macbook" }
        try await repo!.importLibrary(smaller)
        counts = try await repo!.rowCounts()
        #expect(counts.snippets == 2 && counts.groups == 2 && counts.usage == 2)
        try await repo!.importLibrary(fixture)
        counts = try await repo!.rowCounts()
        #expect(counts.snippets == 3 && counts.groups == 3 && counts.usage == 3)
        #expect(try await repo!.loadLibrary() == fixture)
    }

    @Test func interruptedImportLeavesEmptyStoreAndUntouchedSource() async throws {
        let dir = scratchStoreDir("interrupted")
        defer { try? FileManager.default.removeItem(at: dir) }
        // The "source" is a private copy of the frozen fixture file; the import reads it, never writes it.
        let sourceURL = dir.appendingPathComponent("library-source.json")
        try FileManager.default.copyItem(at: frozenLibraryFixtureURL(), to: sourceURL)
        let sourceBytesBefore = try Data(contentsOf: sourceURL)
        let fixture = try KudzuJSON.decode(Library.self, from: sourceBytesBefore)

        var repo: LibraryRepository? = try await openLocalRepository(at: dir)
        await repo!.testingSetFaultBeforeSave(true)
        await #expect(throws: LibraryRepositoryError.injectedFault) {
            try await repo!.importLibrary(fixture)
        }
        var counts = try await repo!.rowCounts()
        #expect(counts.snippets == 0 && counts.groups == 0 && counts.usage == 0)
        #expect(repo!.snapshot == .empty)
        #expect(try await repo!.loadLibrary() == Library(formatVersion: 1))

        repo = nil
        repo = try await openLocalRepository(at: dir)
        counts = try await repo!.rowCounts()
        #expect(counts.snippets == 0 && counts.groups == 0 && counts.usage == 0)
        #expect(try Data(contentsOf: sourceURL) == sourceBytesBefore)

        // The retry completes on the same store.
        try await repo!.importLibrary(fixture)
        counts = try await repo!.rowCounts()
        #expect(counts.snippets == 3 && counts.groups == 3 && counts.usage == 3)
        #expect(repo!.snapshot == fixture)
        #expect(try Data(contentsOf: sourceURL) == sourceBytesBefore)
    }

    // MARK: Rejection and rollback

    @Test func rejectedEditChangesNothing() async throws {
        let dir = scratchStoreDir("rejected")
        defer { try? FileManager.default.removeItem(at: dir) }
        let fixture = try frozenLibraryFixture()
        var repo: LibraryRepository? = try await openLocalRepository(at: dir)
        try await repo!.importLibrary(fixture)

        // A domain rule refuses: moving Work under its own grandchild is a cycle.
        let work: Group = { var g = fixture.groups[0]; g.parentID = fixture.groups[2].id; return g }()
        await #expect(throws: LibraryEditError.groupCycle) {
            try await repo!.edit { try LibraryEditing.saveGroup(work, in: &$0) }
        }
        // A body that mutates and then throws: the mutation is discarded with it.
        struct Refused: Error {}
        await #expect(throws: Refused.self) {
            try await repo!.edit { library in
                LibraryEditing.deleteSnippets(ids: library.snippets.map(\.id), in: &library)
                library.groups.removeAll()
                throw Refused()
            }
        }
        #expect(repo!.snapshot == fixture)
        let counts = try await repo!.rowCounts()
        #expect(counts.snippets == 3 && counts.groups == 3 && counts.usage == 3)
        repo = nil
        let reopened = try await openLocalRepository(at: dir)
        #expect(reopened.snapshot == fixture)
    }

    @Test func faultInsideTransactionRollsBackRowsAndDisk() async throws {
        let dir = scratchStoreDir("rollback")
        defer { try? FileManager.default.removeItem(at: dir) }
        let fixture = try frozenLibraryFixture()
        var repo: LibraryRepository? = try await openLocalRepository(at: dir)
        try await repo!.importLibrary(fixture)

        // Rows are inserted, updated and deleted in the context, then the transaction throws
        // before saving. Everything staged must be discarded.
        let mutation: @Sendable (inout Library) throws -> Void = { library in
            LibraryEditing.createSnippet(in: &library, draft: SnippetDraft(name: "Staged", triggers: [";s"]), id: uuid("30"), now: t2)
            LibraryEditing.deleteSnippets(ids: [fixture.snippets[1].id], in: &library)
            _ = try LibraryEditing.createGroup(name: "Staged group", in: &library, id: uuid("31"))
            _ = try LibraryEditing.saveSnippet({ var s = fixture.snippets[0]; s.name = "Changed"; return s }(), in: &library, now: t2)
        }
        await repo!.testingSetFaultBeforeSave(true)
        await #expect(throws: LibraryRepositoryError.injectedFault) {
            try await repo!.edit(mutation)
        }
        #expect(repo!.snapshot == fixture)
        #expect(try await repo!.loadLibrary() == fixture)                 // the actor's own context, after rollback
        var counts = try await repo!.rowCounts()
        #expect(counts.snippets == 3 && counts.groups == 3 && counts.usage == 3)

        // A second, independent container on the same file sees the committed rows only.
        repo = nil
        repo = try await openLocalRepository(at: dir)
        #expect(repo!.snapshot == fixture)

        // The store is still healthy: the same mutation succeeds once the fault is gone.
        await repo!.testingSetFaultBeforeSave(false)
        try await repo!.edit(mutation)
        var expected = fixture
        try mutation(&expected)
        #expect(repo!.snapshot == expected)
        counts = try await repo!.rowCounts()
        #expect(counts.snippets == 3 && counts.groups == 4)
        repo = nil
        let reopened = try await openLocalRepository(at: dir)
        #expect(reopened.snapshot == expected)
    }

    // MARK: Bad rows are reported, never defaulted

    @Test func unreadableRowsAreReportedWithRowAndField() async throws {
        let dir = scratchStoreDir("badrows")
        defer { try? FileManager.default.removeItem(at: dir) }
        let repo = try await openLocalRepository(at: dir)
        try await repo.importLibrary(try frozenLibraryFixture())

        // Write a damaged row from a second context on the same container (this stands in for
        // "bytes this build did not write"; it is not a sync test).
        let writer = ModelContext(repo.modelContainer)
        let bad = PersistedSnippet()
        bad.id = uuid("40")
        bad.triggersJSON = Data("[\"ok\"]".utf8)
        bad.fireModeRaw = "sideways"
        bad.caseModeRaw = "propagate"
        writer.insert(bad)
        try writer.save()
        await #expect(throws: LibraryRepositoryError.unreadableField(entity: "PersistedSnippet", id: uuid("40").lowercased, field: "fireModeRaw", reason: "unknown value \"sideways\"")) {
            try await repo.loadLibrary()
        }
        // The published snapshot is the last good one; nothing was replaced with a default.
        #expect(repo.snapshot.snippets.count == 3)

        bad.fireModeRaw = "immediate"
        bad.formattingJSON = Data("{\"source\":\"x\",\"spans\":[{\"start\":0,\"end\":1,\"style\":{\"kind\":\"glitter\"}}]}".utf8)
        try writer.save()
        do {
            _ = try await repo.loadLibrary()
            Issue.record("unknown style kind decoded")
        } catch let LibraryRepositoryError.unreadableField(entity, id, field, reason) {
            #expect(entity == "PersistedSnippet" && id == uuid("40").lowercased && field == "formattingJSON")
            #expect(reason.contains("unknown format style glitter"))
        }

        // Two rows with one id: reported, not merged or picked.
        bad.formattingJSON = nil
        let twin = PersistedSnippet()
        twin.id = uuid("40")
        twin.triggersJSON = Data("[]".utf8)
        twin.fireModeRaw = "immediate"
        twin.caseModeRaw = "exact"
        writer.insert(twin)
        try writer.save()
        await #expect(throws: ServiceError.self) {
            try await repo.loadLibrary()
        }
        // And a write is refused too, so the duplicate is never silently "resolved" by an edit.
        await #expect(throws: ServiceError.self) {
            try await repo.edit { LibraryEditing.createSnippet(in: &$0, id: uuid("41"), now: t2) }
        }
        let recoveries = try FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil).filter { $0.lastPathComponent.hasPrefix("library-conflicts-") }
        #expect(!recoveries.isEmpty)
        let recovered = try KudzuJSON.decodeLibrary(String(contentsOf: recoveries[0], encoding: .utf8))
        #expect(recovered.snippets.filter { $0.id == uuid("40") }.count == 2)
    }

    // MARK: Publication

    @Test func changesStreamYieldsCommittedSnapshots() async throws {
        let dir = scratchStoreDir("changes")
        defer { try? FileManager.default.removeItem(at: dir) }
        let repo = try await openLocalRepository(at: dir)
        let stream = repo.changes
        try await repo.edit { LibraryEditing.createSnippet(in: &$0, draft: SnippetDraft(name: "One"), id: uuid("50"), now: t0) }
        var iterator = stream.makeAsyncIterator()
        let first = try #require(await iterator.next())
        #expect(first.snippets.map(\.name) == ["One"])
        #expect(first == repo.snapshot)
        // An edit that changes nothing publishes nothing and writes nothing.
        try await repo.edit { _ in }
        #expect(repo.snapshot == first)
    }
}
