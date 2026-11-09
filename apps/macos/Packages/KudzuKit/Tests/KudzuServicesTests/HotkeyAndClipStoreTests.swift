import Foundation
import KudzuCore
@testable import KudzuServices
import Testing

@Suite struct HotkeyCenterTests {
    // Carbon modifier bits: cmdKey 256, shiftKey 512, optionKey 2048, controlKey 4096.
    static let table: [(String, UInt32, UInt32)] = [
        ("Control+Alt+Space", 49, 4096 | 2048),
        ("Control+Alt+Shift+Space", 49, 4096 | 2048 | 512),
        ("Control+Alt+V", 9, 4096 | 2048),
        ("Control+Alt+Shift+V", 9, 4096 | 2048 | 512),
        ("Command+Shift+Semicolon", 41, 256 | 512),
        ("CmdOrCtrl+K", 40, 256),
        ("Super+Digit1", 18, 256),
        ("Meta+Shift+Enter", 36, 256 | 512),
        ("Option+Slash", 44, 2048),
        ("Ctrl+F5", 96, 4096),
        ("F12", 111, 0),
        ("Control+ArrowUp", 126, 4096),
    ]

    @Test(arguments: table)
    func acceleratorsMapToCarbon(accelerator: String, keyCode: UInt32, modifiers: UInt32) throws {
        let combo = try #require(HotkeyCenter.combo(for: accelerator))
        #expect(combo.keyCode == keyCode)
        #expect(combo.modifiers == modifiers)
    }

    @Test(arguments: ["", "Control+", "Hyper+K", "Control+Banana"])
    func invalidAcceleratorsAreRejected(_ accelerator: String) {
        #expect(HotkeyCenter.combo(for: accelerator) == nil)
    }

    @Test @MainActor func releasingCenterRemovesRegistrationsAndCallback() throws {
        do {
            let center = HotkeyCenter()
            try center.register(.palette, accelerator: "Control+Alt+Shift+F17")
        }
        let replacement = HotkeyCenter()
        try replacement.register(.palette, accelerator: "Control+Alt+Shift+F17")
        replacement.unregister(.palette)
    }

    @Test @MainActor func slotsRefuseEachOthersShortcutAndInvalidOnes() throws {
        let center = HotkeyCenter()
        // An unusual combo so nothing else on the test machine owns it.
        try center.register(.palette, accelerator: "Control+Alt+Shift+F19")
        #expect(center.accelerator(for: .palette) == "Control+Alt+Shift+F19")
        #expect(throws: ServiceError("Kudzu already uses that shortcut for something else.")) {
            try center.register(.clipboard, accelerator: "Control+Shift+Alt+F19")
        }
        #expect(throws: ServiceError.self) { try center.register(.clipboard, accelerator: "Nope+K") }
        #expect(center.accelerator(for: .clipboard) == nil)
        // Re-registering the same shortcut is a no-op; replacing keeps one per slot.
        try center.register(.palette, accelerator: "Control+Alt+Shift+F19")
        try center.register(.palette, accelerator: "Control+Alt+Shift+F18")
        #expect(center.accelerator(for: .palette) == "Control+Alt+Shift+F18")
        try center.register(.clipboard, accelerator: "Control+Alt+Shift+F19")
        center.unregister(.palette)
        center.unregister(.clipboard)
        #expect(center.accelerator(for: .palette) == nil)
    }
}

/// Port of `clipboard/store.rs` tests.
@Suite struct ClipStoreTests {
    private func add(_ store: inout ClipStore, _ text: String) throws -> ClipEntry {
        let items: [Clipboard.PasteItem] = [[(Clipboard.uti.PLAIN_TEXT, Data(text.utf8))]]
        let c = try #require(Clipboard.classify(items))
        let e = store.history.ingest(c, now: Date()).entry
        try Clipboard.encodeItems(items).write(to: store.blobURL(e.id))
        store.markDirty()
        return e
    }

    @Test func persistsAndRestoresThroughTrash() throws {
        let dir = tempDir("clip")
        defer { try? FileManager.default.removeItem(at: dir) }
        var store = ClipStore.load(dir)
        let a = try add(&store, "alpha")
        let b = try add(&store, "beta")
        store.saveIfDirty()
        #expect(store.blobURL(a.id).lastPathComponent == "\(a.id.lowercased).kzc")

        var again = ClipStore.load(dir)
        #expect(again.history.entries.count == 2)
        #expect(again.items(a.id).flatMap(Clipboard.textOf) == "alpha")

        let gone = again.history.remove([b.id])
        again.trash(gone)
        #expect(!FileManager.default.fileExists(atPath: again.blobURL(b.id).path))
        again.untrash(gone)
        #expect(FileManager.default.fileExists(atPath: again.blobURL(b.id).path))

        // Trash is emptied and orphans removed on the next launch.
        again.trash(gone)
        again.saveIfDirty()
        try Data("orphan".utf8).write(to: again.blobURL(UUID()))
        let third = ClipStore.load(dir)
        #expect(try FileManager.default.contentsOfDirectory(atPath: dir.appendingPathComponent("trash").path).isEmpty)
        #expect(try FileManager.default.contentsOfDirectory(atPath: dir.appendingPathComponent("items").path).count == 1, "orphan removed, trashed blob gone")
        #expect(third.diskUsage() > 0)
    }

    @Test func corruptIndexPreservesRecoverableMediaAcrossLaunches() throws {
        let dir = tempDir("clip-recovery")
        defer { try? FileManager.default.removeItem(at: dir) }
        var original = ClipStore.load(dir)
        let clip = try add(&original, "recoverable text")
        original.saveIfDirty()
        write("{broken", to: dir.appendingPathComponent("history.json"))
        var recovered = ClipStore.load(dir)
        #expect(FileManager.default.fileExists(atPath: recovered.blobURL(clip.id).path))
        recovered.markDirty()
        recovered.saveIfDirty()
        let again = ClipStore.load(dir)
        #expect(FileManager.default.fileExists(atPath: again.blobURL(clip.id).path))
    }

    @Test func deletedEntriesLeaveTheTrashEvenWhileARecoveryIsPending() throws {
        let dir = tempDir("clip-trash")
        defer { try? FileManager.default.removeItem(at: dir) }
        write("{broken", to: dir.appendingPathComponent("history-corrupt-1730000000.json")) // left by the Tauri app
        var store = ClipStore.load(dir)
        let secret = try add(&store, "hunter2")
        let gone = store.history.remove([secret.id])
        store.trash(gone)
        store.saveIfDirty()
        _ = ClipStore.load(dir)
        #expect(try FileManager.default.contentsOfDirectory(atPath: dir.appendingPathComponent("trash").path).isEmpty)
    }

    @Test func corruptIndexIsMovedAside() throws {
        let dir = tempDir("clip-corrupt")
        defer { try? FileManager.default.removeItem(at: dir) }
        try Data("{nope".utf8).write(to: dir.appendingPathComponent("history.json"))
        let store = ClipStore.load(dir)
        #expect(store.history.entries.isEmpty)
        #expect(try FileManager.default.contentsOfDirectory(atPath: dir.path).contains { $0.hasPrefix("history-corrupt-") })
        let mode = try FileManager.default.attributesOfItem(atPath: dir.path)[.posixPermissions] as? Int
        #expect(mode == 0o700)
    }
}
