import Foundation

/// Identifiers for the persistent stores. Changing any of these after release orphans user data.
public enum KudzuStoreConstants {
    /// CloudKit container for the synced configuration. Tied to our team, so unsigned builds can't sync.
    public static let cloudKitContainerID = "iCloud.sh.saeed.kudzu"
    /// Apple Developer team that owns `cloudKitContainerID`. Other teams' builds stay local.
    public static let cloudKitTeamID = "V3HN8HXZYK"
    /// Configuration holding snippets, groups, usage and synced settings.
    public static let syncedConfigurationName = "Synced"
    /// Configuration holding per-device settings; never synced.
    public static let localConfigurationName = "Local"
    public static let syncedStoreFileName = "Synced.store"
    public static let localStoreFileName = "Local.store"
    /// Subdirectory of Application Support used when no store directory is given.
    ///
    /// Without it SwiftData would put `Synced.store` directly in
    /// `~/Library/Application Support`, which unsandboxed apps share.
    public static let defaultDirectoryName = "Kudzu"
    /// How long change notifications are coalesced before `KudzuStore.changes` fires.
    public static let changeDebounce: Duration = .milliseconds(100)
}
