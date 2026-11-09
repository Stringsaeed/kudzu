import Foundation

/// Who this copy of Kudzu is: bundle id, version and the folders that follow from them.
///
/// The release build is `sh.saeed.kudzu`; Kudzu Dev is `sh.saeed.kudzu.dev`, with its own data,
/// permissions, hotkeys and MCP server name. Both read the same files as the Tauri app.
public struct AppIdentity: Sendable, Hashable {
    public static let productionID = "sh.saeed.kudzu"
    public static let devID = "sh.saeed.kudzu.dev"
    /// Pre-release Tauri builds (`store::migrate_legacy_dir`).
    public static let legacyID = "sh.saeed.kudzu.desktop"

    public var bundleID: String
    /// `CFBundleShortVersionString` (the Tauri app's `package.json` version).
    public var version: String
    /// "Kudzu" or "Kudzu Dev".
    public var productName: String

    public init(bundleID: String, version: String, productName: String) {
        self.bundleID = bundleID
        self.version = version
        self.productName = productName
    }

    /// The running bundle. A bare executable (tests, `swift run`) counts as Kudzu Dev.
    public static var current: AppIdentity {
        let bundle = Bundle.main
        let id = bundle.bundleIdentifier ?? devID
        let version = bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "0.0.0"
        let name = bundle.object(forInfoDictionaryKey: "CFBundleName") as? String ?? (id == devID ? "Kudzu Dev" : "Kudzu")
        return AppIdentity(bundleID: id, version: version, productName: name)
    }

    /// `lib.rs` `is_dev_identifier`.
    public static func isDevIdentifier(_ identifier: String) -> Bool { identifier == devID }

    public var isDev: Bool { Self.isDevIdentifier(bundleID) }

    /// `~/Library/Application Support/<bundle id>/` (Tauri's `app_data_dir`).
    public var dataDirectory: URL { Self.dataDirectory(for: bundleID) }

    /// `~/Library/Logs/<bundle id>/` (tauri-plugin-log's `LogDir`).
    public var logDirectory: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Logs", isDirectory: true)
            .appendingPathComponent(bundleID, isDirectory: true)
    }

    public static func dataDirectory(for bundleID: String) -> URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support", isDirectory: true)
            .appendingPathComponent(bundleID, isDirectory: true)
    }
}
