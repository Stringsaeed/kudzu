import Foundation
import KudzuCore
@testable import KudzuEngine
import Testing

// Pasteboard item blobs and the platform tests of `macos/{clipboard,apps}.rs` that need no
// permission (they read installed bundles and draw icons; nothing touches the general pasteboard).
@Suite struct ClipboardTests {
    private func same(_ a: [EngineClipboard.PasteItem], _ b: [EngineClipboard.PasteItem]) -> Bool {
        a.count == b.count && zip(a, b).allSatisfy { x, y in
            x.count == y.count && zip(x, y).allSatisfy { $0.0 == $1.0 && $0.1 == $1.1 }
        }
    }

    @Test func pasteItemsRoundTripThroughTheBlobFormat() throws {
        let items: [EngineClipboard.PasteItem] = [
            [("public.utf8-plain-text", Data("héllo 👋🏽".utf8)), ("public.html", Data("<b>héllo</b>".utf8))],
            [("public.png", Data([0x89, 0x50, 0x4E, 0x47, 0, 1, 2, 255]))],
            [("org.nspasteboard.ConcealedType", Data())],
        ]
        let blob = Clipboard.encodeItems(items)
        let decoded = try #require(Clipboard.decodeItems(blob))
        #expect(same(decoded, items))
        #expect(same(try #require(Clipboard.decodeItems(Clipboard.encodeItems([]))), []))
        #expect(Clipboard.decodeItems(blob + Data([0])) == nil)
        #expect(Clipboard.decodeItems(blob.dropLast()) == nil)
    }

    @Test func findsSystemAppsSortedAndUnique() {
        let apps = Apps.installedApps()
        if apps.isEmpty {
            return // Not a regular macOS install (CI sandbox).
        }
        let ids = apps.map(\.bundleId)
        #expect(Set(ids).count == ids.count)
        let names = apps.map { $0.name.lowercased() }
        #expect(zip(names, names.dropFirst()).allSatisfy { !$1.utf8.lexicographicallyPrecedes($0.utf8) })
    }

    @Test func iconIsPNG() throws {
        guard let app = Apps.installedApps().first(where: { $0.path != nil }), let path = app.path else { return }
        let png = try #require(Apps.iconPNG(path: path, size: 32))
        #expect(Array(png.prefix(8)) == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
        #expect(Apps.iconPNG(path: "/definitely/not/here.app", size: 32) == nil)
    }

    @Test func thumbnailsScaleDownAndReportPixelSize() throws {
        guard let app = Apps.installedApps().first(where: { $0.path != nil }), let path = app.path else { return }
        let png = try #require(Apps.iconPNG(path: path, size: 256))
        let thumb = try #require(EngineClipboard.thumbnail(data: png, maxPx: 64))
        #expect(thumb.width == 256 && thumb.height == 256)
        #expect(Array(thumb.png.prefix(8)) == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
        let small = try #require(EngineClipboard.thumbnail(data: thumb.png, maxPx: 1024))
        #expect(small.width == 64 && small.height == 64)
        #expect(EngineClipboard.thumbnail(data: Data("not an image".utf8), maxPx: 64) == nil)
    }

    @Test func localeHasNoKeywordSuffix() {
        let locale = System.locale()
        #expect(!locale.isEmpty)
        #expect(!locale.contains("@"))
    }
}
