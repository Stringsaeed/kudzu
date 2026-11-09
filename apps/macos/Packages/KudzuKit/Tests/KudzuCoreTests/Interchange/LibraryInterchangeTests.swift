import Foundation
import Testing
@testable import KudzuCore

@Suite struct LibraryInterchangeTests {
    func at(_ raw: String = "2026-11-01T14:30:05Z") -> Date { KudzuDate.parse(raw)! }
    func s(_ name: String, _ trigger: String, _ content: String) -> Snippet { Snippet(name:name,triggers:[trigger],content:content,createdAt:at()) }
    func store(_ dir: URL) -> BackupStore { BackupStore(dir:dir,timeZone:.gmt) }
    @Test func jsonEncodesSortedPrettyIsoDatesAndRoundTrips() throws {
        let date = Date(timeIntervalSince1970:1_762_000_000), gid = UUID(uuidString:"11111111-2222-3333-4444-555555555555")!, sid = UUID(uuidString:"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")!
        let group = Group(id:gid,name:"Work",insertion:.paste,rule:AppRule(mode:.allow,bundleIDs:["com.apple.mail"],insertion:.paste)), snippet = Snippet(id:sid,name:"Signature",triggers:[";sig",":sig"],content:"Best regards,\n{{cursor}}",groupID:gid,fireMode:.wholeWord,insertion:.automatic,isFavorite:true,createdAt:date)
        let lib = Library(groups:[group],snippets:[snippet],usage:[UsageRecord(snippetID:sid,deviceID:"mac-1",count:42,charsSaved:500,lastUsedAt:date)]), json = try KudzuJSON.encodeLibrary(lib)
        #expect(json.contains("\n")); #expect(json.range(of:"\"formatVersion\"")!.lowerBound < json.range(of:"\"groups\"")!.lowerBound); #expect(json.range(of:"\"groups\"")!.lowerBound < json.range(of:"\"snippets\"")!.lowerBound); #expect(json.range(of:"\"snippets\"")!.lowerBound < json.range(of:"\"usage\"")!.lowerBound); #expect(json.range(of:"\"caseMode\"")!.lowerBound < json.range(of:"\"content\"")!.lowerBound); #expect(json.contains("2025-11-01T12:26:40Z")); #expect(try KudzuJSON.decodeLibrary(json) == lib)
    }
    @Test func jsonRejectsNewerFormatVersions() throws { #expect(throws:InterchangeError.unsupportedFormatVersion(found:2,maxSupported:1)) { try KudzuJSON.decodeLibrary(KudzuJSON.encodeLibrary(Library(formatVersion:2))) }; #expect(throws:(any Error).self) { try KudzuJSON.decodeLibrary("{nope") } }
    @Test func backupWritesExpectedFilenameAndLoadsBack() throws {
        let dir = try t2Temp(); defer { try? FileManager.default.removeItem(at:dir) }; let store = store(dir), lib = Library(snippets:[s("Test",";t","Hello")]), info = try store.write(lib,reason:.preImport,now:at())
        #expect(info.filename == "kudzu-backup-2026-11-01T143005-preImport.json"); let list = try store.list(); #expect(list.count == 1); #expect(list[0] == info); #expect(info.createdAt == at()); #expect(info.reason == .preImport); #expect(try store.load(URL(fileURLWithPath:info.path)) == lib)
    }
    @Test func backupListsNewestFirstAndPrunes() throws {
        let dir = try t2Temp(); defer { try? FileManager.default.removeItem(at:dir) }; let store = store(dir)
        for day in 1...17 { _ = try store.write(.empty,reason:day%3 == 0 ? .manual : .daily,now:at(String(format:"2026-10-%02dT09:00:00Z",day))) }
        let all = try store.list(); #expect(all.count == 17); #expect(all.first?.filename == "kudzu-backup-2026-10-17T090000-daily.json"); #expect(all.last?.filename == "kudzu-backup-2026-10-01T090000-daily.json"); #expect(try store.prune(keep:14).count == 3); #expect(try store.list().count == 14); #expect(try store.list().last?.filename == "kudzu-backup-2026-10-04T090000-daily.json")
    }
    @Test func needsDailyBackupUsesTheStoreTimeZone() throws {
        let dir = try t2Temp(); defer { try? FileManager.default.removeItem(at:dir) }; let store = BackupStore(dir:dir,timeZone:TimeZone(secondsFromGMT:-5*3600)!), morning = at("2026-11-01T08:15:00-05:00"), evening = at("2026-11-01T21:45:00-05:00"), next = at("2026-11-02T00:05:00-05:00")
        #expect(store.needsDailyBackup(now:morning)); _ = try store.write(.empty,reason:.preImport,now:morning); #expect(store.needsDailyBackup(now:evening)); let daily = try store.write(.empty,reason:.daily,now:morning); #expect(daily.filename == "kudzu-backup-2026-11-01T081500-daily.json"); #expect(!store.needsDailyBackup(now:evening)); #expect(store.needsDailyBackup(now:next))
    }
    @Test func sameSecondBackupsAreNeverOverwritten() throws {
        let dir = try t2Temp(); defer { try? FileManager.default.removeItem(at:dir) }; let store = store(dir), now = at("2026-11-01T12:00:00Z"), a = try store.write(Library(snippets:[s("Original",";o","o")]),reason:.manual,now:now), b = try store.write(Library(snippets:[s("Restored",";r","r")]),reason:.manual,now:now)
        #expect(a.path != b.path); #expect(b.filename == "kudzu-backup-2026-11-01T120000-manual.2.json"); #expect(try store.list().count == 2); #expect(try store.list()[0] == b); #expect(try store.load(URL(fileURLWithPath:a.path)).snippets[0].name == "Original")
    }
    @Test func backupListIgnoresForeignFilesAndMissingDirs() throws {
        let dir = try t2Temp(); defer { try? FileManager.default.removeItem(at:dir) }; #expect(try store(dir.appendingPathComponent("nope")).list().isEmpty)
        for name in ["notes.txt","kudzu-backup-bad-daily.json","kudzu-backup-2026-11-01T120000-weird.json"] { try Data("{}".utf8).write(to:dir.appendingPathComponent(name)) }; #expect(try store(dir).list().isEmpty)
    }
    @Test func groupsWarningsByMessageMostFrequentFirst() { let grouped = Coordinator.groupWarnings([ImportWarning(file:"c.yml",line:1,message:"Image match skipped."),ImportWarning(file:"a.yml",line:1,message:"Markdown match imported as plain text."),ImportWarning(file:"b.yml",line:1,message:"Markdown match imported as plain text.")]); #expect(grouped.count == 2); #expect(grouped[0].message == "Markdown match imported as plain text."); #expect(grouped.map(\.count) == [2,1]) }
    @Test func detectsAndPreviewsEspansoDirsInHome() throws {
        let home = try t2Temp(); defer { try? FileManager.default.removeItem(at:home) }; let dir = home.appendingPathComponent("Library/Application Support/espanso/match"); try FileManager.default.createDirectory(at:dir,withIntermediateDirectories:true); try Data("matches:\n  - trigger: \":hi\"\n    replace: \"Hello there\"\n  - trigger: \":bye\"\n    replace: \"Goodbye\"\n".utf8).write(to:dir.appendingPathComponent("base.yml")); let detected = Coordinator.detectEspansoDirs(home:home); #expect(detected.count == 1); #expect(detected.first?.snippetCount == 2); #expect(detected.first?.groupCount == 1)
    }
    @Test func importEspansoBacksUpMergesDedupesAndSummarizes() throws {
        let dir = try t2Temp(); defer { try? FileManager.default.removeItem(at:dir) }; let file = dir.appendingPathComponent("base.yml")
        try Data("matches:\n  - trigger: \";hi\"\n    replace: \"Hello\"\n  - trigger: \";conflict\"\n    replace: \"Incoming conflict\"\n  - trigger: \";md1\"\n    markdown: \"**Bold 1**\"\n  - trigger: \";md2\"\n    markdown: \"**Bold 2**\"\n".utf8).write(to:file)
        let existing = Library(snippets:[s("Existing Hi",";hi","Hello"),s("Existing Conflict",";conflict","Existing")]), backups = store(dir.appendingPathComponent("Backups")), result = Espanso.importFiles([file]), merged = try Coordinator.importEspanso(existing:existing,result:result,backups:backups,now:at())
        #expect(merged.summary.importedSnippets == 4); #expect(merged.summary.importedGroups == 1); #expect(merged.summary.dedupeReport?.mergedSnippets == 1); #expect(merged.summary.dedupeReport?.conflicts.map(\.trigger) == [";conflict"]); #expect(merged.summary.warnings.count == 2); #expect(merged.summary.groupedWarnings?.count == 1); #expect(merged.summary.groupedWarnings?.first?.count == 2); #expect(try backups.list().count == 1); #expect(merged.library.snippets.count == 5)
    }
    @Test func importKudzuJsonThenRestoreWithSafetyBackup() throws {
        let dir = try t2Temp(); defer { try? FileManager.default.removeItem(at:dir) }; let backups = store(dir.appendingPathComponent("Backups")), original = Library(snippets:[s("Original",";orig","Original content")]), incoming = Library(groups:[Group(name:"Imported Group")],snippets:[s("New",";new","New content")]), merged = try Coordinator.importKudzuJSON(existing:original,json:KudzuJSON.encodeLibrary(incoming),backups:backups,now:Date(timeIntervalSince1970:1_700_000_000))
        #expect(merged.summary.importedSnippets == 1); #expect(merged.library.snippets.count == 2); #expect(merged.library.groups.count == 1)
        let before = try backups.list(); #expect(before.count == 1); let restored = try Coordinator.restoreBackup(current:merged.library,path:URL(fileURLWithPath:before[0].path),backups:backups,now:Date(timeIntervalSince1970:1_700_000_100)); #expect(restored.library.snippets.map(\.triggers) == [[";orig"]]); #expect(try backups.list().count == 2); #expect(try backups.load(URL(fileURLWithPath:restored.safety.path)).snippets.count == 2)
    }
    @Test func mergePrefersNewerEditsTakesUsageMaximaAndDropsBadLinks() {
        let t0 = at("2026-01-01T00:00:00Z"), t1 = at("2026-01-02T00:00:00Z"), root = Group(name:"Root"), snippet = Snippet(name:"Old",triggers:[";a"],content:"A",createdAt:t0), child = Group(name:"Child",parentID:root.id), orphan = Group(name:"Orphan",parentID:UUID())
        let existing = Library(groups:[root],snippets:[snippet],usage:[UsageRecord(snippetID:snippet.id,deviceID:"m",count:5,charsSaved:10,lastUsedAt:t0)])
        var edit = snippet; edit.name = "New"; edit.updatedAt = t1; edit.groupID = UUID(); var stale = snippet; stale.name = "Stale"
        let incoming = Library(groups:[child,orphan],snippets:[edit,stale],usage:[UsageRecord(snippetID:snippet.id,deviceID:"m",count:3,charsSaved:20,lastUsedAt:t1)]), result = Coordinator.mergeLibrary(existing:existing,incoming:incoming)
        #expect(result.report.isEmpty); #expect(result.library.groups.first { $0.id == child.id }?.parentID == root.id); #expect(result.library.groups.first { $0.id == orphan.id }?.parentID == nil); #expect(result.library.snippets.count == 1); #expect(result.library.snippets[0].name == "New"); #expect(result.library.snippets[0].groupID == nil); #expect(result.library.usage == [UsageRecord(snippetID:snippet.id,deviceID:"m",count:5,charsSaved:20,lastUsedAt:t1)])
    }
    @Test func mergeBreaksParentCycles() { var a = Group(name:"A"); let b = Group(name:"B",parentID:a.id); a.parentID = b.id; let r = Coordinator.mergeLibrary(existing:.empty,incoming:Library(groups:[a,b])); #expect(r.library.groups.first { $0.id == a.id }?.parentID == b.id); #expect(r.library.groups.first { $0.id == b.id }?.parentID == nil) }
    @Test func exportEspansoAndEnsureGettingStartedOnce() throws {
        let dir = try t2Temp(); defer { try? FileManager.default.removeItem(at:dir) }; var library = Library()
        let first = Coordinator.ensureGettingStartedSnippets(&library,now:at()), second = Coordinator.ensureGettingStartedSnippets(&library,now:at()); #expect(first && !second); #expect(library.groups.map(\.name) == ["Getting Started"]); #expect(library.snippets.count == 3); #expect(library.snippets.first { $0.triggers == [";kudzu"] }?.content == "Kudzu is working!")
        let files = try Coordinator.exportEspansoToDir(library,dir:dir); #expect(files.count == 1); #expect(files.first?.lastPathComponent == "Getting Started.yml"); let r = try Espanso.importDir(dir); #expect(r.library.snippets.count == 3); #expect(r.warnings.isEmpty)
    }
    @Test func gettingStartedReusesAnExistingGroupAndRespectsExistingSample() {
        var library = Library(groups:[Group(name:"getting started")]); let changed = Coordinator.ensureGettingStartedSnippets(&library,now:at()); #expect(changed); #expect(library.groups.count == 1); #expect(library.snippets.allSatisfy { $0.groupID == library.groups[0].id })
        var taken = Library(snippets:[s("Mine",";KUDZU","mine")]); let added = Coordinator.ensureGettingStartedSnippets(&taken,now:at()); #expect(!added); #expect(taken.snippets.count == 1)
    }
}
