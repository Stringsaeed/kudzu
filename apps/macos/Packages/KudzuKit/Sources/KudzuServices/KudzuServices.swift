// KudzuServices — everything the Tauri shell (`apps/desktop/src-tauri/src`) did outside the
// webviews, as plain Swift types. See apps/macos/docs/MIGRATION.md §4.3.
//
// | Type               | Port of                                   |
// |--------------------|-------------------------------------------|
// | `AppIdentity`      | `lib.rs` (`is_dev`), tauri.conf.json      |
// | `Logging`          | tauri-plugin-log setup in `lib.rs`        |
// | `AtomicFile`       | `store::write_atomic`                     |
// | `DataStore`        | `store.rs`, `settings.rs`                 |
// | `ClipStore`        | `clipboard/store.rs`                      |
// | `ClipboardHistory` | `clipboard/mod.rs`, `clipboard/protocol.rs` |
// | `HotkeyCenter`     | `hotkey.rs`                               |
// | `LoginItem`        | tauri-plugin-autostart                    |
// | `Updater`          | `updater.rs` + tauri-plugin-updater       |
// | `CloudSync`        | `cloud/{mod,apple}.rs`                    |
// | `AgentServer`      | `agent.rs`                                |
// | `PermissionGuide`  | `permissions/{mod.rs,macos.m}`            |
import Foundation

/// A failed service call, carrying the message the UI shows (the Tauri command's error string).
public struct ServiceError: Error, Equatable, Sendable, CustomStringConvertible, LocalizedError {
    public var message: String
    public init(_ message: String) { self.message = message }
    public var description: String { message }
    public var errorDescription: String? { message }
}
