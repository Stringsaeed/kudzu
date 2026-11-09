import AppKit
import Foundation
import KudzuCore
import KudzuEngine
import KudzuServices

// Templates, apps, import/export and backups (`commands.rs`).
extension LiveBackend {
    // MARK: Templates

    public func renderPreview(content: String, snippetId: UUID?, formValues: [String: String], formatting: Formatting?) async throws -> RenderResult {
        render(content: content, snippetId: snippetId, formValues: formValues, formatting: formatting)
    }

    func render(content: String, snippetId: UUID?, formValues: [String: String], formatting: Formatting?) -> RenderResult {
        let snippets = Dictionary(store.library.snippets.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        let context = RenderContext(
            now: Date(),
            timeZone: .current,
            locale: Locale(identifier: System.locale()),
            clipboard: System.readClipboard(),
            formValues: formValues,
            resolveSnippet: { snippets[$0]?.content },
            resolveFormatting: { snippets[$0]?.formatting },
            rootSnippetID: snippetId
        )
        return TemplateRenderer.renderFormatted(content, formatting: formatting, context: context)
    }

    public func copyPreview(content: String, snippetId: UUID?, formatting: Formatting?) async throws {
        copyRendered(render(content: content, snippetId: snippetId, formValues: [:], formatting: formatting))
    }

    /// Both clipboard flavours: plain text, plus HTML when the result is formatted.
    func copyRendered(_ result: RenderResult) {
        var item: Clipboard.PasteItem = [(Clipboard.uti.PLAIN_TEXT, Data(result.text.utf8))]
        if let spans = result.formatting {
            item.append((Clipboard.uti.HTML, Data(Formatting.html(result.text, spans: spans).utf8)))
        }
        EngineClipboard.write([item])
    }

    // MARK: Apps

    public func listApps() async throws -> [AppInfo] {
        await Task.detached(priority: .userInitiated) { System.installedApps() }.value
    }

    public func appIcon(path: String) async throws -> Data? {
        await Task.detached(priority: .userInitiated) { System.appIconPNG(path: path, size: 64) }.value
    }

    // MARK: Import / export / backups

    public func detectEspanso() async throws -> [EspansoDetection] {
        await Task.detached(priority: .userInitiated) {
            Coordinator.detectEspansoDirs(home: FileManager.default.homeDirectoryForCurrentUser)
                .filter { $0.snippetCount > 0 }
                .map { EspansoDetection(path: $0.path.path, snippetCount: $0.snippetCount, groupCount: $0.groupCount) }
        }.value
    }

    /// Parse off the UI actor, then merge under the store lock so concurrent agent/CloudKit
    /// edits cannot be overwritten between reading the current library and applying the import.
    private func importData(_ load: @escaping @Sendable () throws -> ImportResult) async throws -> ImportSummary {
        let store = self.store
        do {
            let parsed = try await Task.detached(priority: .userInitiated) { try load() }.value
            let summary = try await store.repository.edit { library in
                let committed = library
                let result = try Coordinator.importResult(existing: library, result: parsed, backups: store.backups, now: Date(), safetyLibrary: committed)
                library = result.library
                return result.summary
            }
            return summary
        } catch { throw backendError(error) }
    }

    public func importEspanso(path: String) async throws -> ImportSummary {
        try await importData {
            let url = URL(fileURLWithPath: path)
            var isDir: ObjCBool = false
            FileManager.default.fileExists(atPath: path, isDirectory: &isDir)
            return isDir.boolValue ? try Espanso.importDir(url) : Espanso.importFiles([url])
        }
    }

    public func importFile(path: String) async throws -> ImportSummary {
        try await importData { try Coordinator.loadImport(URL(fileURLWithPath: path)) }
    }

    /// Exports and backups encode the committed library only: pending saves are flushed first
    /// (off the main actor), a save that fails is not exported, and the private `_nativeSync`
    /// state never leaves `library.json`.
    public func exportJson(path: String) async throws {
        do {
            let store = self.store
            try await Task.detached(priority: .userInitiated) {
                store.flush()
                let json = try KudzuJSON.encodeLibrary(store.library)
                try AtomicFile.write(Data(json.utf8), to: URL(fileURLWithPath: path))
            }.value
        } catch {
            throw backendError(error)
        }
    }

    public func exportEspanso(dir: String) async throws -> Int {
        do {
            let store = self.store
            return try await Task.detached(priority: .userInitiated) {
                store.flush()
                return try Coordinator.exportEspansoToDir(store.library, dir: URL(fileURLWithPath: dir)).count
            }.value
        } catch {
            throw backendError(error)
        }
    }

    public func listBackups() async throws -> [BackupInfo] {
        do {
            let backups = store.backups
            return try await Task.detached(priority: .userInitiated) {
                try backups.list().sorted { $0.createdAt > $1.createdAt }
            }.value
        } catch {
            throw backendError(error)
        }
    }

    public func backupNow() async throws -> BackupInfo {
        do {
            let backups = store.backups
            let store = self.store
            return try await Task.detached(priority: .userInitiated) {
                store.flush()
                let info = try backups.write(store.library, reason: .manual, now: Date())
                _ = try? backups.prune(keep: 14)
                return info
            }.value
        } catch {
            throw backendError(error)
        }
    }

    public func restoreBackup(path: String) async throws {
        do {
            let store = self.store
            try await store.repository.edit { library in
                let restored = try Coordinator.restoreBackup(current: library, path: URL(fileURLWithPath: path), backups: store.backups, now: Date(), safetyLibrary: library)
                library = restored.library
            }
        } catch {
            throw backendError(error)
        }
    }

    public func revealDataFolder() async throws {
        NSWorkspace.shared.activateFileViewerSelecting([store.dir.appendingPathComponent("library.json")])
    }
}
