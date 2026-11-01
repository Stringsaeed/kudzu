import Foundation
import SwiftData

/// Settings shared across the user's devices. A singleton by convention: use `KudzuStore.syncedSettings()`.
///
/// Two devices can each create one before they sync; dedupe keeps the most
/// recently edited (`updatedAt`, nil = never edited) and deletes the rest.
@Model
public final class SyncedSettingsModel {
    public var id: UUID = UUID()
    /// Typing speed used to estimate time saved.
    public var typingSpeedWPM: Int = 40
    /// Prefix suggested for new snippet triggers.
    public var defaultTriggerPrefix: String = ";"
    /// Last user edit; nil for untouched defaults so they never win over real settings from another device.
    public var updatedAt: Date?

    public init() {}
}

/// Per-device settings. Stored in the local-only configuration; never synced.
@Model
public final class LocalSettingsModel {
    public var id: UUID = UUID()
    /// Identifies this device in usage records. Generated once.
    public var deviceID: String = ""
    /// Whether the synced configuration uses CloudKit. Takes effect on next container creation.
    public var syncEnabled: Bool = true
    /// Search-palette hotkey, serialized by the app layer. Placeholder until the hotkey format is defined.
    public var paletteHotkey: String?

    public init(deviceID: String) {
        self.deviceID = deviceID
    }
}
