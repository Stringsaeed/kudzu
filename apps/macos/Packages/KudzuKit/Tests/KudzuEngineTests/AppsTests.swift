import Foundation
import KudzuCore
@testable import KudzuEngine
import Testing

@Suite struct AppsTests {
    @Test func orderingUsesNameBytesBeforeBundleId() {
        let composed = AppInfo(bundleId: "a", name: "\u{e9}")
        let decomposed = AppInfo(bundleId: "z", name: "e\u{301}")
        #expect([composed, decomposed].sorted(by: Apps.orderedBefore).map(\.bundleId) == ["z", "a"])
        #expect(Apps.orderedBefore(AppInfo(bundleId: "a", name: "Notes"), AppInfo(bundleId: "z", name: "NOTES")))
    }

    @Test func scansOnePlainSubfolderButNeverDescendsIntoAppBundles() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        for relative in ["Top.app/Contents/Hidden.app", "Tools/Nested.APP", "Tools/Deeper/Ignored.app"] {
            try FileManager.default.createDirectory(at: root.appendingPathComponent(relative), withIntermediateDirectories: true)
        }
        #expect(Set(Apps.appBundles(root).map(\.lastPathComponent)) == ["Top.app", "Nested.APP"])
    }

    @Test func readsBundleNamesAndRejectsMissingIdentifiers() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString).appendingPathComponent("Fallback.app")
        defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
        let contents = root.appendingPathComponent("Contents")
        try FileManager.default.createDirectory(at: contents, withIntermediateDirectories: true)
        let plist = contents.appendingPathComponent("Info.plist")
        func write(_ dict: [String: String]) throws {
            try PropertyListSerialization.data(fromPropertyList: dict, format: .xml, options: 0).write(to: plist)
        }
        try write(["CFBundleIdentifier": " com.example.app \n", "CFBundleDisplayName": " Display ", "CFBundleName": "Name"])
        #expect(Apps.readBundle(root)?.name == "Display")
        #expect(Apps.readBundle(root)?.bundleId == "com.example.app")
        try write(["CFBundleIdentifier": "com.example.app", "CFBundleDisplayName": "  "])
        #expect(Apps.readBundle(root)?.name == "Fallback")
        try write(["CFBundleName": "Missing ID"])
        #expect(Apps.readBundle(root) == nil)
    }
}
