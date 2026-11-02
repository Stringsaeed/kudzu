import Foundation

/// Why this build cannot use the Kudzu iCloud container.
public enum CloudSyncBlock: String, Equatable, Sendable {
    /// Ad-hoc or unsigned: no team owns the container.
    case unsignedBuild
    /// Signed by a team other than the one that owns the container.
    case differentTeam
    /// Signed by our team, but the signature has no entitlement for the container.
    case missingEntitlement
}

/// Decides whether a build may open the store with CloudKit. The app supplies the signature facts.
public enum CloudSyncEligibility {
    /// Nil when sync is allowed.
    public static func block(
        teamID: String?,
        containerIdentifiers: [String],
        ownerTeamID: String = KudzuStoreConstants.cloudKitTeamID,
        containerID: String = KudzuStoreConstants.cloudKitContainerID
    ) -> CloudSyncBlock? {
        guard let teamID, !teamID.isEmpty else { return .unsignedBuild }
        if teamID != ownerTeamID { return .differentTeam }
        if !containerIdentifiers.contains(containerID) { return .missingEntitlement }
        return nil
    }
}

/// The per-Mac iCloud switch, read before the store opens.
///
/// `LocalSettingsModel.syncEnabled` lives inside the store, and the store's CloudKit flag is
/// fixed when the container is created. UserDefaults is the copy consulted at launch. The app
/// mirrors a change into local settings so the two don't drift.
public enum SyncPreference {
    public static let defaultsKey = "sh.saeed.kudzu.icloudSyncEnabled"

    /// True when the key has never been set: sync is on unless the user turns it off.
    public static func isEnabled(in defaults: UserDefaults) -> Bool {
        guard defaults.object(forKey: defaultsKey) != nil else { return true }
        return defaults.bool(forKey: defaultsKey)
    }

    public static func setEnabled(_ enabled: Bool, in defaults: UserDefaults) {
        defaults.set(enabled, forKey: defaultsKey)
    }
}

/// iCloud account, as far as the status UI cares.
public enum SyncAccountState: Equatable, Sendable {
    case unknown
    case available
    case noAccount
    case restricted
    case temporarilyUnavailable
}

/// A CloudKit mirroring activity still running, or just finished.
public enum SyncActivity: Equatable, Sendable, Hashable {
    case settingUp
    case importing
    case exporting
}

/// One `NSPersistentCloudKitContainer` event, stripped of Core Data types so the fold is testable.
public struct CloudSyncEvent: Equatable, Sendable {
    public enum Kind: Equatable, Sendable {
        case setup
        case `import`
        case export
    }

    public var kind: Kind
    /// False while `endDate` is still nil.
    public var finished: Bool
    public var succeeded: Bool
    public var errorDescription: String?
    public var date: Date

    public init(kind: Kind, finished: Bool, succeeded: Bool, errorDescription: String?, date: Date) {
        self.kind = kind
        self.finished = finished
        self.succeeded = succeeded
        self.errorDescription = errorDescription
        self.date = date
    }

    public var activity: SyncActivity {
        switch kind {
        case .setup: .settingUp
        case .import: .importing
        case .export: .exporting
        }
    }

    /// A finished successful import is the signal to dedupe. Exports and setup don't change local rows,
    /// and SwiftData does not always post `NSPersistentStoreRemoteChange` for a CloudKit import.
    public var shouldNoteRemoteChange: Bool {
        kind == .import && finished && succeeded
    }
}

/// What the menu bar and Settings show about iCloud.
public enum SyncPhase: Equatable, Sendable {
    case unavailable(CloudSyncBlock)
    /// The switch was flipped; the open container still has the previous flag.
    case needsRestart(turningOn: Bool)
    case off
    case noAccount
    case restricted
    case temporarilyUnavailable
    case syncing(Set<SyncActivity>)
    /// `lastSyncedAt` is nil until the first successful import, export, or setup.
    case upToDate(lastSyncedAt: Date?)
    case failed(String)

    /// One line for the menu bar and the Sync settings row.
    public var statusText: String {
        switch self {
        case .unavailable(.unsignedBuild):
            "iCloud sync needs a build signed by the Kudzu developer"
        case .unavailable(.differentTeam):
            "iCloud sync isn't available in this build"
        case .unavailable(.missingEntitlement):
            "This build isn't entitled to the Kudzu iCloud container"
        case .needsRestart(true):
            "iCloud sync turns on after you quit and reopen Kudzu"
        case .needsRestart(false):
            "iCloud sync turns off after you quit and reopen Kudzu"
        case .off:
            "iCloud sync is off"
        case .noAccount:
            "Sign in to iCloud to sync your snippets"
        case .restricted:
            "iCloud is restricted on this Mac"
        case .temporarilyUnavailable:
            "iCloud is temporarily unavailable"
        case .syncing(let activities):
            if activities.contains(.importing) && activities.contains(.exporting) {
                "Syncing with iCloud…"
            } else if activities.contains(.importing) {
                "Downloading snippet changes…"
            } else if activities.contains(.exporting) {
                "Uploading snippet changes…"
            } else {
                "Preparing iCloud sync…"
            }
        case .upToDate(nil):
            "iCloud sync is on"
        case .upToDate(let date?):
            "Synced with iCloud \(date.formatted(.relative(presentation: .named)))"
        case .failed(let message):
            message
        }
    }
}

/// Facts the status line is folded from. The app fills these in; `phase` decides what to show.
public struct SyncSnapshot: Equatable, Sendable {
    public var preferenceEnabled: Bool
    /// Whether the container that is open right now is mirroring to CloudKit.
    public var runningEnabled: Bool
    public var blocked: CloudSyncBlock?
    /// Set when the synced container could not be opened and the app fell back to a local store.
    public var openError: String?
    /// Set when CloudKit rejects the container (missing entitlement, unknown container).
    public var accountError: String?
    public var account: SyncAccountState
    public var activities: Set<SyncActivity>
    public var lastSuccessAt: Date?
    public var lastError: String?
    public var lastErrorAt: Date?

    public init(
        preferenceEnabled: Bool,
        runningEnabled: Bool,
        blocked: CloudSyncBlock? = nil,
        openError: String? = nil,
        accountError: String? = nil,
        account: SyncAccountState = .unknown,
        activities: Set<SyncActivity> = [],
        lastSuccessAt: Date? = nil,
        lastError: String? = nil,
        lastErrorAt: Date? = nil
    ) {
        self.preferenceEnabled = preferenceEnabled
        self.runningEnabled = runningEnabled
        self.blocked = blocked
        self.openError = openError
        self.accountError = accountError
        self.account = account
        self.activities = activities
        self.lastSuccessAt = lastSuccessAt
        self.lastError = lastError
        self.lastErrorAt = lastErrorAt
    }

    public var phase: SyncPhase {
        if let blocked { return .unavailable(blocked) }
        if let openError, preferenceEnabled, !runningEnabled { return .failed(openError) }
        if preferenceEnabled != runningEnabled { return .needsRestart(turningOn: preferenceEnabled) }
        guard runningEnabled else { return .off }
        if let accountError { return .failed(accountError) }
        switch account {
        case .noAccount: return .noAccount
        case .restricted: return .restricted
        case .temporarilyUnavailable: return .temporarilyUnavailable
        case .unknown, .available: break
        }
        if !activities.isEmpty { return .syncing(activities) }
        if let lastError {
            if let lastSuccessAt, let lastErrorAt, lastSuccessAt >= lastErrorAt {
                return .upToDate(lastSyncedAt: lastSuccessAt)
            }
            return .failed(lastError)
        }
        return .upToDate(lastSyncedAt: lastSuccessAt)
    }
}

/// Folds container events into a snapshot. Account and preference changes are applied by the app.
public enum SyncSnapshotReducer {
    public static func apply(_ event: CloudSyncEvent, to snapshot: SyncSnapshot) -> SyncSnapshot {
        var next = snapshot
        if !event.finished {
            next.activities.insert(event.activity)
            return next
        }
        next.activities.remove(event.activity)
        if event.succeeded {
            next.lastSuccessAt = event.date
            if let lastErrorAt = next.lastErrorAt, event.date < lastErrorAt {
                return next
            }
            next.lastError = nil
            next.lastErrorAt = nil
        } else {
            let trimmed = event.errorDescription?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            next.lastError = trimmed.isEmpty ? "iCloud sync failed." : trimmed
            next.lastErrorAt = event.date
        }
        return next
    }
}
