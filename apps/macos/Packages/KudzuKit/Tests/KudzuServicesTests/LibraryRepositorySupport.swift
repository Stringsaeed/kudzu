import Foundation
import KudzuCore
import SwiftData
import Testing
@testable import KudzuServices

// Shared fixtures for the SwiftData prototype tests. Everything on disk goes into a fresh folder
// under the per-user temporary directory and is removed by the test that made it.

/// A fresh scratch folder for one test's store files.
func scratchStoreDir(_ name: String) -> URL {
    let url = FileManager.default.temporaryDirectory
        .appendingPathComponent("kudzu-swiftdata-\(name)-\(UUID().uuidString.prefix(8))", isDirectory: true)
    try! FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}

/// The frozen 0.6.0 library fixture (`Tests/KudzuCoreTests/Fixtures/models/library.json`), read only.
func frozenLibraryFixtureURL() -> URL {
    URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()            // KudzuServicesTests
        .deletingLastPathComponent()            // Tests
        .appendingPathComponent("KudzuCoreTests/Fixtures/models/library.json")
}

func frozenLibraryFixture() throws -> Library {
    try KudzuJSON.decode(Library.self, from: Data(contentsOf: frozenLibraryFixtureURL()))
}

func uuid(_ suffix: String) -> UUID {
    UUID(uuidString: "0000aaaa-bbbb-4ccc-8ddd-" + String(repeating: "0", count: 12 - suffix.count) + suffix)!
}

/// Fixed instants so every date assertion is literal.
let t0 = Date(timeIntervalSince1970: 1_762_600_000)            // 2025-11-08T...Z, whole second
let t1 = Date(timeIntervalSince1970: 1_762_600_000.123456)     // sub-second, as `library-minimal.json` parses
let t2 = Date(timeIntervalSince1970: 1_762_700_000)

/// 1x1 PNG, the smallest payload the editor could embed.
let onePixelPNG = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=="

/// Explicit edge cases the frozen fixture does not cover: Unicode normalization forms that
/// compare equal as `String` but differ in bytes, control characters, long image payloads,
/// unsorted list order, deep ancestry, and per-device usage with a negative total.
enum EdgeLibrary {
    static let nfc = "caf\u{E9}"                 // é precomposed
    static let nfd = "cafe\u{301}"               // e + combining acute
    static let zwj = "👩\u{200D}💻 \u{FEFF}\r\n\ttab\u{0}end"

    static let root = Group(id: uuid("1"), name: "Root", parentID: nil, sortIndex: 5, isEnabled: true, insertion: .paste, rule: AppRule(mode: .allow, bundleIDs: ["com.apple.mail", "com.apple.Notes"], insertion: .type))
    static let child = Group(id: uuid("2"), name: nfd, parentID: root.id, sortIndex: 0, isEnabled: false)
    static let grandchild = Group(id: uuid("3"), name: nfc, parentID: child.id, sortIndex: 0, insertion: .type, rule: AppRule(mode: .deny, bundleIDs: []))
    static let sibling = Group(id: uuid("4"), name: "Root", parentID: nil, sortIndex: 5)   // same name & sortIndex as root

    static let bigImage = "data:image/png;base64," + String(repeating: "QUJDREVGR0g=", count: 20_000)  // ~240 KB

    static let rich = Snippet(
        id: uuid("a"),
        name: nfd,
        triggers: [";" + nfc, ";" + nfd, "", "  ", "x"],
        content: "Hello **world** [Logo] \u{200B} " + zwj,
        formatting: Formatting(source: "Hello **world** [Logo] \u{200B} " + zwj, spans: [
            FormatSpan(start: 0, end: 5, style: .bold),
            FormatSpan(start: 6, end: 15, style: .italic),
            FormatSpan(start: 6, end: 15, style: .underline),
            FormatSpan(start: 0, end: 5, style: .link(url: "https://例え.jp/path?q=a&b=c#frag")),
            FormatSpan(start: 16, end: 22, style: .image(dataUrl: onePixelPNG, width: 1)),
            FormatSpan(start: 23, end: 24, style: .image(dataUrl: bigImage, width: 512)),
        ]),
        groupID: grandchild.id,
        fireMode: .wholeWord,
        caseMode: .exact,
        insertion: .type,
        rule: AppRule(mode: .deny, bundleIDs: ["com.tinyspeck.slackmacgap"], insertion: .paste),
        isFavorite: true,
        isEnabled: false,
        createdAt: t1,
        updatedAt: t0
    )
    static let emptyFormatting = Snippet(id: uuid("b"), name: "", triggers: [], content: "", formatting: Formatting(source: "", spans: []), groupID: nil, createdAt: t0, updatedAt: t2)
    static let plain = Snippet(id: uuid("c"), name: "Plain", triggers: [";p"], content: "{{date format=\"yyyy\"}} \\{{literal}}", groupID: uuid("9") /* dangling: no such group */, createdAt: t2, updatedAt: t2)

    static let usage = [
        UsageRecord(snippetID: rich.id, deviceID: "mac-studio", count: 128, charsSaved: 4096, lastUsedAt: t1),
        UsageRecord(snippetID: rich.id, deviceID: "macbook", count: 3, charsSaved: -2, lastUsedAt: nil),
        UsageRecord(snippetID: plain.id, deviceID: "", count: 0, charsSaved: 0, lastUsedAt: t0),
        UsageRecord(snippetID: uuid("f"), deviceID: "mac-studio", count: 1, charsSaved: 1, lastUsedAt: t2),  // usage for a snippet that is gone
    ]

    /// Groups out of sortIndex order, snippets out of createdAt order: only `listOrder` can keep this.
    static let library = Library(groups: [grandchild, root, sibling, child], snippets: [plain, rich, emptyFormatting], usage: usage)
}

/// Opens a repository on a scratch store with CloudKit mirroring explicitly off.
func openLocalRepository(at dir: URL) async throws -> LibraryRepository {
    try await LibraryRepository.open(.local(url: dir.appendingPathComponent("Library.store")))
}

func utf8(_ s: String) -> [UInt8] { Array(s.utf8) }
