import Foundation
import KudzuCore
import KudzuEngine
@testable import KudzuServices
import Testing

/// A pasteboard in memory.
final class FakePasteboard: ClipboardSystem, @unchecked Sendable {
    // @unchecked: every property is only touched while holding `lock`.
    private let lock = NSLock()
    private var count = 0
    private var own: Set<Int> = []
    private var items: [Clipboard.PasteItem] = []
    private var front: AppInfo? = AppInfo(bundleId: "com.apple.TextEdit", name: "TextEdit")
    private(set) var written: [[Clipboard.PasteItem]] = []

    func copy(_ text: String, from app: AppInfo? = AppInfo(bundleId: "com.apple.TextEdit", name: "TextEdit"), extra: [(String, Data)] = []) {
        lock.withLock {
            count += 1
            items = [[(Clipboard.uti.PLAIN_TEXT, Data(text.utf8))] + extra]
            front = app
        }
    }

    func changeCount() -> Int { lock.withLock { count } }
    func isOwnChange(_ count: Int) -> Bool { lock.withLock { own.contains(count) } }
    func read() -> [Clipboard.PasteItem] { lock.withLock { items } }
    func write(_ items: [Clipboard.PasteItem]) {
        lock.withLock {
            count += 1
            own.insert(count)
            self.items = items
            written.append(items)
        }
    }
    func canPaste() -> Bool { false }
    func paste(target: AppInfo?) throws(EngineClipboard.PasteError) { throw .notAllowed }
    func thumbnail(_ data: Data, maxPx: Int) -> EngineClipboard.Thumbnail? { nil }
    func frontmostApp() -> AppInfo? { lock.withLock { front } }
    func iconPNG(path: String, size: Int) -> Data? { Data("png".utf8) }
}

final class EventLog: @unchecked Sendable {
    private let lock = NSLock()
    private var list: [ClipEvent] = []
    var events: [ClipEvent] { lock.withLock { list } }
    var add: @Sendable (ClipEvent) -> Void { { [self] e in lock.withLock { list.append(e) } } }
}

@Suite struct ClipboardHistoryTests {
    private func wait(_ condition: () -> Bool) async {
        for _ in 0 ..< 40 where !condition() { try? await Task.sleep(for: .milliseconds(100)) }
    }

    @Test func recordsCopiesAndSkipsOwnPrivateAndIgnored() async throws {
        let dir = tempDir("history")
        defer { try? FileManager.default.removeItem(at: dir) }
        let board = FakePasteboard()
        board.copy("already there")
        let log = EventLog()
        let history = ClipboardHistory(dir: dir, system: board, config: {
            ClipboardHistory.Config(enabled: true, ignoredApps: ["com.1password.1password"], retentionDays: 7)
        }, emit: log.add)
        history.start()
        defer { history.stop() }
        try await Task.sleep(for: .milliseconds(400))
        #expect(history.entries.isEmpty, "what's on the clipboard at launch isn't a new copy")

        board.copy("hello world")
        await wait { history.entries.count == 1 }
        let entry = try #require(history.entries.first)
        #expect(entry.text == "hello world")
        #expect(entry.source?.bundleId == "com.apple.TextEdit")
        #expect(history.text(entry.id) == "hello world")
        #expect(FileManager.default.fileExists(atPath: dir.appendingPathComponent("items/\(entry.id.lowercased).kzc").path))
        if case .upsert = log.events.first {} else { Issue.record("expected an upsert event") }

        board.copy("p4ssw0rd!", from: AppInfo(bundleId: "com.1password.1password", name: "1Password"))
        board.copy("concealed", extra: [("org.nspasteboard.ConcealedType", Data())])
        try await Task.sleep(for: .milliseconds(700))
        #expect(history.entries.count == 1, "ignored apps and concealed copies are skipped")

        // Putting an entry back is Kudzu's own change: not recorded again, but marked used.
        try history.copy(entry.id, plain: true)
        try await Task.sleep(for: .milliseconds(700))
        #expect(history.entries.count == 1)
        #expect(history.entries.first?.useCount == 1)
        #expect(board.written.last?.first?.first?.0 == Clipboard.uti.PLAIN_TEXT)

        // The index is saved within a second.
        await wait { FileManager.default.fileExists(atPath: dir.appendingPathComponent("history.json").path) }
        #expect(ClipStore.load(dir).history.entries.count == 1)
    }

    @Test func commandsAndMedia() throws {
        let dir = tempDir("history-cmds")
        defer { try? FileManager.default.removeItem(at: dir) }
        let board = FakePasteboard()
        let log = EventLog()
        let history = ClipboardHistory(dir: dir, system: board, config: {
            ClipboardHistory.Config(enabled: true, ignoredApps: [], retentionDays: 7)
        }, emit: log.add)
        board.copy("one")
        history.capture(front: board.frontmostApp())
        board.copy("two")
        history.capture(front: board.frontmostApp())
        let ids = history.entries.map(\.id)
        #expect(ids.count == 2)

        #expect(history.pin(ids[0], pinned: true)?.pinned == true)
        let removed = history.delete([ids[1]])
        #expect(removed.count == 1)
        history.restore(removed)
        #expect(history.entries.count == 2)
        #expect(throws: ServiceError("That item is no longer in your clipboard history.")) { try history.copy(UUID(), plain: false) }
        #expect(throws: ServiceError("Not found")) { try history.snippetText(UUID()) }
        #expect(try history.snippetText(ids[1]) == "one")
        #expect(history.media(kind: .thumb, id: ids[0], index: 0) == nil)
        #expect(history.media(kind: .icon, id: ids[0], index: 0) == nil, "not a file entry")
        #expect(history.clear(includePinned: false) == 1)
        #expect(history.entries.map(\.id) == [ids[0]], "pinned entries stay")
        #expect(history.list().entries.count == 1)
    }
}
