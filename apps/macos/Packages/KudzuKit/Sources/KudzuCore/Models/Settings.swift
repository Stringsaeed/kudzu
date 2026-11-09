import Foundation

// Port of `src-tauri/src/settings.rs`: app settings persisted as `settings.json`.

/// Appearance preference.
public enum Theme: String, Codable, Hashable, Sendable, CaseIterable {
    case dark
    case light
    case system
}

/// Which update feed to follow.
public enum UpdateChannel: String, Codable, Hashable, Sendable, CaseIterable {
    case stable
    /// Pre-releases too (and stable releases whenever they're newer).
    case beta
}

/// Everything the user can configure outside the library itself.
///
/// Decodes with `#[serde(default)]` semantics: every missing key takes its value from
/// ``default`` (or from the decoder's ``Foundation/CodingUserInfoKey/kudzuSettingsDefaults``,
/// see ``KudzuJSON/decoder(settingsDefaults:)``). In SwiftUI files, SwiftUI's `Settings` scene
/// shadows this name; KudzuApp declares its own alias.
public struct Settings: Codable, Hashable, Sendable {
    public var typingSpeedWPM: UInt32
    public var defaultTriggerPrefix: String
    /// Accelerator string, e.g. "Control+Alt+Space".
    public var paletteHotkey: String
    public var paused: Bool
    public var defaultInsertion: InsertionMethod
    /// Apps where nothing ever expands.
    public var deniedBundleIDs: [String]
    /// Per-app insertion overrides.
    public var appInsertions: [String: InsertionMethod]
    public var launchAtLogin: Bool
    public var onboardingCompleted: Bool
    public var theme: Theme
    /// Show the dock icon while the main window is open.
    public var showInDock: Bool
    /// Look for new versions in the background (release builds only).
    public var autoCheckUpdates: Bool
    /// "beta" also receives pre-releases.
    public var updateChannel: UpdateChannel
    /// Sync the library through iCloud (release builds signed for the container).
    public var icloudSync: Bool
    /// The app version that last ran with this data; a change means "just updated". Written as `null` when unset.
    public var lastRunVersion: String?
    /// Record clipboard history.
    public var clipboardEnabled: Bool
    /// Days to keep unpinned clipboard history; 0 keeps it until the size cap.
    public var clipboardRetentionDays: UInt32
    /// Global shortcut for the clipboard history picker.
    public var clipboardHotkey: String
    /// Apps whose copies are never recorded.
    public var clipboardIgnoredApps: [String]
    /// Paste formatted text as plain text by default (⇧ flips it).
    public var clipboardPastePlain: Bool
    /// Show passwords, keys and tokens in clipboard history without hovering.
    public var clipboardShowSecrets: Bool
    /// Answer AI agents (MCP) on the local socket.
    public var agentAccess: Bool
    /// Agents may create and edit snippets.
    public var agentWrite: Bool
    /// Agents may search clipboard history (secrets are always withheld).
    public var agentClipboard: Bool

    // MARK: Defaults

    /// `Settings::default()`.
    public static let `default` = Settings(
        typingSpeedWPM: 40,
        defaultTriggerPrefix: ";",
        paletteHotkey: "Control+Alt+Space",
        paused: false,
        defaultInsertion: .automatic,
        deniedBundleIDs: MatchingConfig.defaultDeniedBundleIDs.sorted(),
        appInsertions: MatchingConfig.defaultAppInsertions,
        launchAtLogin: false,
        onboardingCompleted: false,
        theme: .dark,
        showInDock: true,
        autoCheckUpdates: true,
        updateChannel: .stable,
        icloudSync: true,
        lastRunVersion: nil,
        clipboardEnabled: true,
        clipboardRetentionDays: ClipEntry.defaultRetentionDays,
        clipboardHotkey: "Control+Alt+V",
        clipboardIgnoredApps: ClipEntry.defaultIgnoredApps,
        clipboardPastePlain: false,
        clipboardShowSecrets: false,
        agentAccess: false,
        agentWrite: true,
        agentClipboard: true
    )

    /// The development variant's defaults (`Store::load_dev`): paused, and hotkeys that don't
    /// clash with an installed release build.
    public static let devDefault: Settings = {
        var s = Settings.default
        s.paused = true
        s.paletteHotkey = "Control+Alt+Shift+Space"
        s.clipboardHotkey = "Control+Alt+Shift+V"
        return s
    }()

    public init(
        typingSpeedWPM: UInt32,
        defaultTriggerPrefix: String,
        paletteHotkey: String,
        paused: Bool,
        defaultInsertion: InsertionMethod,
        deniedBundleIDs: [String],
        appInsertions: [String: InsertionMethod],
        launchAtLogin: Bool,
        onboardingCompleted: Bool,
        theme: Theme,
        showInDock: Bool,
        autoCheckUpdates: Bool,
        updateChannel: UpdateChannel,
        icloudSync: Bool,
        lastRunVersion: String?,
        clipboardEnabled: Bool,
        clipboardRetentionDays: UInt32,
        clipboardHotkey: String,
        clipboardIgnoredApps: [String],
        clipboardPastePlain: Bool,
        clipboardShowSecrets: Bool,
        agentAccess: Bool,
        agentWrite: Bool,
        agentClipboard: Bool
    ) {
        self.typingSpeedWPM = typingSpeedWPM
        self.defaultTriggerPrefix = defaultTriggerPrefix
        self.paletteHotkey = paletteHotkey
        self.paused = paused
        self.defaultInsertion = defaultInsertion
        self.deniedBundleIDs = deniedBundleIDs
        self.appInsertions = appInsertions
        self.launchAtLogin = launchAtLogin
        self.onboardingCompleted = onboardingCompleted
        self.theme = theme
        self.showInDock = showInDock
        self.autoCheckUpdates = autoCheckUpdates
        self.updateChannel = updateChannel
        self.icloudSync = icloudSync
        self.lastRunVersion = lastRunVersion
        self.clipboardEnabled = clipboardEnabled
        self.clipboardRetentionDays = clipboardRetentionDays
        self.clipboardHotkey = clipboardHotkey
        self.clipboardIgnoredApps = clipboardIgnoredApps
        self.clipboardPastePlain = clipboardPastePlain
        self.clipboardShowSecrets = clipboardShowSecrets
        self.agentAccess = agentAccess
        self.agentWrite = agentWrite
        self.agentClipboard = agentClipboard
    }

    // MARK: Codable

    private enum CodingKeys: String, CodingKey {
        case typingSpeedWPM, defaultTriggerPrefix, paletteHotkey, paused, defaultInsertion, deniedBundleIDs
        case appInsertions, launchAtLogin, onboardingCompleted, theme, showInDock, autoCheckUpdates
        case updateChannel, icloudSync, lastRunVersion, clipboardEnabled, clipboardRetentionDays
        case clipboardHotkey, clipboardIgnoredApps, clipboardPastePlain, clipboardShowSecrets
        case agentAccess, agentWrite, agentClipboard
    }

    public init(from decoder: any Decoder) throws {
        let d = decoder.userInfo[.kudzuSettingsDefaults] as? Settings ?? .default
        let c = try decoder.container(keyedBy: CodingKeys.self)
        typingSpeedWPM = try c.decode(UInt32.self, forKey: .typingSpeedWPM, default: d.typingSpeedWPM)
        defaultTriggerPrefix = try c.decode(String.self, forKey: .defaultTriggerPrefix, default: d.defaultTriggerPrefix)
        paletteHotkey = try c.decode(String.self, forKey: .paletteHotkey, default: d.paletteHotkey)
        paused = try c.decode(Bool.self, forKey: .paused, default: d.paused)
        defaultInsertion = try c.decode(InsertionMethod.self, forKey: .defaultInsertion, default: d.defaultInsertion)
        deniedBundleIDs = try c.decode([String].self, forKey: .deniedBundleIDs, default: d.deniedBundleIDs)
        appInsertions = try c.decode([String: InsertionMethod].self, forKey: .appInsertions, default: d.appInsertions)
        launchAtLogin = try c.decode(Bool.self, forKey: .launchAtLogin, default: d.launchAtLogin)
        onboardingCompleted = try c.decode(Bool.self, forKey: .onboardingCompleted, default: d.onboardingCompleted)
        theme = try c.decode(Theme.self, forKey: .theme, default: d.theme)
        showInDock = try c.decode(Bool.self, forKey: .showInDock, default: d.showInDock)
        autoCheckUpdates = try c.decode(Bool.self, forKey: .autoCheckUpdates, default: d.autoCheckUpdates)
        updateChannel = try c.decode(UpdateChannel.self, forKey: .updateChannel, default: d.updateChannel)
        icloudSync = try c.decode(Bool.self, forKey: .icloudSync, default: d.icloudSync)
        // serde: a present `null` is `None`; a missing key takes the default.
        lastRunVersion = c.contains(.lastRunVersion) ? try c.decodeIfPresent(String.self, forKey: .lastRunVersion) : d.lastRunVersion
        clipboardEnabled = try c.decode(Bool.self, forKey: .clipboardEnabled, default: d.clipboardEnabled)
        clipboardRetentionDays = try c.decode(UInt32.self, forKey: .clipboardRetentionDays, default: d.clipboardRetentionDays)
        clipboardHotkey = try c.decode(String.self, forKey: .clipboardHotkey, default: d.clipboardHotkey)
        clipboardIgnoredApps = try c.decode([String].self, forKey: .clipboardIgnoredApps, default: d.clipboardIgnoredApps)
        clipboardPastePlain = try c.decode(Bool.self, forKey: .clipboardPastePlain, default: d.clipboardPastePlain)
        clipboardShowSecrets = try c.decode(Bool.self, forKey: .clipboardShowSecrets, default: d.clipboardShowSecrets)
        agentAccess = try c.decode(Bool.self, forKey: .agentAccess, default: d.agentAccess)
        agentWrite = try c.decode(Bool.self, forKey: .agentWrite, default: d.agentWrite)
        agentClipboard = try c.decode(Bool.self, forKey: .agentClipboard, default: d.agentClipboard)
    }

    public func encode(to encoder: any Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(typingSpeedWPM, forKey: .typingSpeedWPM)
        try c.encode(defaultTriggerPrefix, forKey: .defaultTriggerPrefix)
        try c.encode(paletteHotkey, forKey: .paletteHotkey)
        try c.encode(paused, forKey: .paused)
        try c.encode(defaultInsertion, forKey: .defaultInsertion)
        try c.encode(deniedBundleIDs, forKey: .deniedBundleIDs)
        try c.encode(appInsertions, forKey: .appInsertions)
        try c.encode(launchAtLogin, forKey: .launchAtLogin)
        try c.encode(onboardingCompleted, forKey: .onboardingCompleted)
        try c.encode(theme, forKey: .theme)
        try c.encode(showInDock, forKey: .showInDock)
        try c.encode(autoCheckUpdates, forKey: .autoCheckUpdates)
        try c.encode(updateChannel, forKey: .updateChannel)
        try c.encode(icloudSync, forKey: .icloudSync)
        try c.encodeNullable(lastRunVersion, forKey: .lastRunVersion)
        try c.encode(clipboardEnabled, forKey: .clipboardEnabled)
        try c.encode(clipboardRetentionDays, forKey: .clipboardRetentionDays)
        try c.encode(clipboardHotkey, forKey: .clipboardHotkey)
        try c.encode(clipboardIgnoredApps, forKey: .clipboardIgnoredApps)
        try c.encode(clipboardPastePlain, forKey: .clipboardPastePlain)
        try c.encode(clipboardShowSecrets, forKey: .clipboardShowSecrets)
        try c.encode(agentAccess, forKey: .agentAccess)
        try c.encode(agentWrite, forKey: .agentWrite)
        try c.encode(agentClipboard, forKey: .agentClipboard)
    }
}
