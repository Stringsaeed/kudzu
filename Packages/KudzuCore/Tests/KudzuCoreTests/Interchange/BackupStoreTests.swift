import Foundation
import Testing
@testable import KudzuCore

@Suite("BackupStore Tests")
struct BackupStoreTests {
    private func makeTempDir() throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("kudzu-backup-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func makeCalendar(timeZoneID: String = "UTC") -> Calendar {
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(identifier: timeZoneID)!
        return cal
    }

    private func makeDate(_ year: Int, _ month: Int, _ day: Int, _ hour: Int, _ minute: Int, _ second: Int, calendar: Calendar) -> Date {
        var comps = DateComponents()
        comps.calendar = calendar
        comps.timeZone = calendar.timeZone
        comps.year = year
        comps.month = month
        comps.day = day
        comps.hour = hour
        comps.minute = minute
        comps.second = second
        return calendar.date(from: comps)!
    }

    @Test("Writes backup with expected filename format and loads it back")
    func writeAndLoad() throws {
        let tempDir = try makeTempDir()
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let cal = makeCalendar()
        let store = BackupStore(directory: tempDir, calendar: cal)
        let now = makeDate(2026, 11, 1, 14, 30, 5, calendar: cal)

        let snippet = SnippetRecord(
            name: "Test",
            triggers: [";t"],
            content: "Hello",
            createdAt: now,
            updatedAt: now
        )
        let library = KudzuLibrary(snippets: [snippet])

        let fileURL = try store.write(library, reason: .preImport, now: now)
        #expect(fileURL.lastPathComponent == "kudzu-backup-2026-11-01T143005-preImport.json")

        let list = try store.list()
        #expect(list.count == 1)
        #expect(list[0].filename == "kudzu-backup-2026-11-01T143005-preImport.json")
        #expect(list[0].reason == .preImport)
        #expect(list[0].createdAt == now)

        let loaded = try store.load(list[0])
        #expect(loaded == library)
    }

    @Test("Lists backups newest first and prunes keeping last 14")
    func listOrderingAndPrune() throws {
        let tempDir = try makeTempDir()
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let cal = makeCalendar()
        let store = BackupStore(directory: tempDir, calendar: cal)
        let library = KudzuLibrary()

        // Create 17 backups across consecutive days
        for day in 1...17 {
            let date = makeDate(2026, 10, day, 9, 0, 0, calendar: cal)
            let reason: BackupReason = (day % 3 == 0) ? .manual : .daily
            try store.write(library, reason: reason, now: date)
        }

        let all = try store.list()
        #expect(all.count == 17)
        // Newest first: Oct 17 down to Oct 1
        #expect(all.first?.filename == "kudzu-backup-2026-10-17T090000-daily.json")
        #expect(all.last?.filename == "kudzu-backup-2026-10-01T090000-daily.json")

        let pruned = try store.prune(keep: 14)
        #expect(pruned.count == 3)

        let remaining = try store.list()
        #expect(remaining.count == 14)
        // Oldest remaining should be Oct 4
        #expect(remaining.last?.filename == "kudzu-backup-2026-10-04T090000-daily.json")
    }

    @Test("Checks needsDailyBackup in the given calendar and timezone")
    func needsDailyBackupCheck() throws {
        let tempDir = try makeTempDir()
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let cal = makeCalendar(timeZoneID: "America/New_York")
        let store = BackupStore(directory: tempDir, calendar: cal)
        let library = KudzuLibrary()

        let morning = makeDate(2026, 11, 1, 8, 15, 0, calendar: cal)
        let evening = makeDate(2026, 11, 1, 21, 45, 0, calendar: cal)
        let nextDay = makeDate(2026, 11, 2, 0, 5, 0, calendar: cal)

        // Initially needs a daily backup
        #expect(store.needsDailyBackup(now: morning) == true)

        // A preImport or manual backup on the same day does NOT satisfy daily backup
        try store.write(library, reason: .preImport, now: morning)
        #expect(store.needsDailyBackup(now: evening) == true)

        // Writing a daily backup satisfies needsDailyBackup for the rest of that calendar day
        try store.write(library, reason: .daily, now: morning)
        #expect(store.needsDailyBackup(now: evening) == false)

        // Next calendar day needs a new daily backup
        #expect(store.needsDailyBackup(now: nextDay) == true)
    }
}
