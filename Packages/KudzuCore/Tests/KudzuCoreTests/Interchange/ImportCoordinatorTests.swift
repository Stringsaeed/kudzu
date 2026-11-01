import Foundation
import Testing
@testable import KudzuCore

@Suite("ImportCoordinator Tests")
@MainActor
struct ImportCoordinatorTests {
    private func makeTempDir() throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("kudzu-import-coordinator-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    @Test("Groups warnings by message sorted by count descending")
    func groupWarningsByMessage() {
        let w1 = ImportWarning(file: "a.yml", line: 1, trigger: ";a", message: "Markdown match imported as plain text.")
        let w2 = ImportWarning(file: "b.yml", line: 2, trigger: ";b", message: "Markdown match imported as plain text.")
        let w3 = ImportWarning(file: "c.yml", line: 3, trigger: ";c", message: "Image match skipped.")

        let grouped = ImportCoordinator.groupWarnings([w3, w1, w2])
        #expect(grouped.count == 2)
        #expect(grouped[0].message == "Markdown match imported as plain text.")
        #expect(grouped[0].count == 2)
        #expect(grouped[1].message == "Image match skipped.")
        #expect(grouped[1].count == 1)
    }

    @Test("Detects Espanso config directories in a home directory and previews snippet count")
    func detectAndPreviewEspansoDirectories() throws {
        let tempHome = try makeTempDir()
        defer { try? FileManager.default.removeItem(at: tempHome) }

        let espansoMatchDir = tempHome.appendingPathComponent("Library/Application Support/espanso/match", isDirectory: true)
        try FileManager.default.createDirectory(at: espansoMatchDir, withIntermediateDirectories: true)

        let yaml = """
        matches:
          - trigger: ":hi"
            replace: "Hello there"
          - trigger: ":bye"
            replace: "Goodbye"
        """
        try Data(yaml.utf8).write(to: espansoMatchDir.appendingPathComponent("base.yml"))

        let detected = ImportCoordinator.detectEspansoDirectories(homeDirectory: tempHome)
        #expect(detected.count == 1)
        #expect(detected[0].snippetCount == 2)
        #expect(detected[0].groupCount == 1)
    }

    @Test("Imports Espanso directory: writes preImport backup, merges, dedupes, and summarizes warnings")
    func importEspansoWritesBackupAndMerges() throws {
        let tempDir = try makeTempDir()
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let backupDir = tempDir.appendingPathComponent("Backups", isDirectory: true)
        let espansoDir = tempDir.appendingPathComponent("espanso/match", isDirectory: true)
        try FileManager.default.createDirectory(at: espansoDir, withIntermediateDirectories: true)

        let yaml = """
        matches:
          - trigger: ";hi"
            replace: "Hello"
          - trigger: ";conflict"
            replace: "Incoming conflict"
          - trigger: ";md1"
            markdown: "**Bold 1**"
          - trigger: ";md2"
            markdown: "**Bold 2**"
        """
        try Data(yaml.utf8).write(to: espansoDir.appendingPathComponent("base.yml"))

        let store = try KudzuStore.inMemory()
        // Pre-populate store with a duplicate (;hi -> "Hello") and a conflicting trigger (;conflict -> "Existing")
        try store.upsert(SnippetRecord(name: "Existing Hi", triggers: [";hi"], content: "Hello"))
        try store.upsert(SnippetRecord(name: "Existing Conflict", triggers: [";conflict"], content: "Existing"))

        let backupStore = BackupStore(directory: backupDir)
        let summary = try ImportCoordinator.importEspanso(
            from: espansoDir,
            into: store,
            backupStore: backupStore
        )

        #expect(summary.importedSnippets == 4)
        #expect(summary.importedGroups == 1)
        #expect(summary.dedupeReport.mergedSnippets == 1)
        #expect(summary.dedupeReport.conflicts.map(\.trigger) == [";conflict"])
        #expect(summary.warnings.count == 2)
        #expect(summary.groupedWarnings.count == 1)
        #expect(summary.groupedWarnings[0].count == 2)

        // Verify preImport backup was created with the original 2 snippets
        let backups = try backupStore.list()
        #expect(backups.count == 1)
        #expect(backups[0].reason == .preImport)
        let backedUpLibrary = try backupStore.load(backups[0])
        #expect(backedUpLibrary.snippets.count == 2)

        // Store now has 5 surviving snippets (2 original + 4 imported - 1 merged duplicate)
        let finalLibrary = try store.library()
        #expect(finalLibrary.snippets.count == 5)
    }

    @Test("Imports Kudzu JSON library with preImport backup and restores backup with manual safety backup")
    func importKudzuJSONAndRestoreBackup() throws {
        let tempDir = try makeTempDir()
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let backupDir = tempDir.appendingPathComponent("Backups", isDirectory: true)
        let backupStore = BackupStore(directory: backupDir)
        let store = try KudzuStore.inMemory()

        try store.upsert(SnippetRecord(name: "Original", triggers: [";orig"], content: "Original content"))

        let incomingLibrary = KudzuLibrary(
            groups: [GroupRecord(name: "Imported Group")],
            snippets: [SnippetRecord(name: "New", triggers: [";new"], content: "New content")]
        )
        let jsonData = try KudzuJSON.encode(incomingLibrary)
        let jsonURL = tempDir.appendingPathComponent("library.json")
        try jsonData.write(to: jsonURL)

        let t1 = Date(timeIntervalSince1970: 1_700_000_000)
        let summary = try ImportCoordinator.importKudzuJSON(
            from: jsonURL,
            into: store,
            backupStore: backupStore,
            now: t1
        )
        #expect(summary.importedSnippets == 1)
        #expect(try store.library().snippets.count == 2)

        // Restore the pre-import backup; should write a .manual safety backup first
        let backupsBeforeRestore = try backupStore.list()
        #expect(backupsBeforeRestore.count == 1)

        let t2 = Date(timeIntervalSince1970: 1_700_000_100)
        try ImportCoordinator.restoreBackup(
            backupsBeforeRestore[0],
            into: store,
            backupStore: backupStore,
            now: t2
        )

        #expect(try store.library().snippets.map(\.triggers) == [[";orig"]])
        let backupsAfterRestore = try backupStore.list()
        #expect(backupsAfterRestore.count == 2)
        #expect(backupsAfterRestore[0].reason == .manual)
        #expect(try backupStore.load(backupsAfterRestore[0]).snippets.count == 2)
    }

    @Test("Exports library to Espanso directory and creates Getting Started snippets once")
    func exportEspansoAndEnsureGettingStarted() throws {
        let tempDir = try makeTempDir()
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let store = try KudzuStore.inMemory()
        let createdFirst = try ImportCoordinator.ensureGettingStartedSnippets(in: store)
        #expect(createdFirst == true)

        let createdSecond = try ImportCoordinator.ensureGettingStartedSnippets(in: store)
        #expect(createdSecond == false)

        let lib = try store.library()
        #expect(lib.groups.map(\.name) == ["Getting Started"])
        #expect(lib.snippets.count == 3)

        let exportDir = tempDir.appendingPathComponent("exported-espanso", isDirectory: true)
        let files = try ImportCoordinator.exportEspanso(library: lib, to: exportDir)
        #expect(files.count == 1)
        #expect(files[0].lastPathComponent == "Getting Started.yml")

        let reimported = try EspansoImporter.importDirectory(exportDir)
        #expect(reimported.library.snippets.count == 3)
    }
}
