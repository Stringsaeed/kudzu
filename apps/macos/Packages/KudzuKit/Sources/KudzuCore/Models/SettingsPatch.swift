import Foundation

/// A partial ``Settings`` update (`Partial<Settings>` passed to `api.updateSettings`).
/// Each non-`nil` field replaces the setting; `lastRunVersion` is doubly optional so a patch can
/// set it to `nil`.
public struct SettingsPatch: Hashable, Sendable {
    public var typingSpeedWPM: UInt32?
    public var defaultTriggerPrefix: String?
    public var paletteHotkey: String?
    public var paused: Bool?
    public var defaultInsertion: InsertionMethod?
    public var deniedBundleIDs: [String]?
    public var appInsertions: [String: InsertionMethod]?
    public var launchAtLogin: Bool?
    public var onboardingCompleted: Bool?
    public var theme: Theme?
    public var showInDock: Bool?
    public var autoCheckUpdates: Bool?
    public var updateChannel: UpdateChannel?
    public var icloudSync: Bool?
    public var lastRunVersion: String??
    public var clipboardEnabled: Bool?
    public var clipboardRetentionDays: UInt32?
    public var clipboardHotkey: String?
    public var clipboardIgnoredApps: [String]?
    public var clipboardPastePlain: Bool?
    public var clipboardShowSecrets: Bool?
    public var agentAccess: Bool?
    public var agentWrite: Bool?
    public var agentClipboard: Bool?

    public init() {}

    /// A patch built by mutating an empty one: `SettingsPatch { $0.paused = true }`.
    public init(_ build: (inout SettingsPatch) -> Void) {
        build(&self)
    }

    /// `settings` with every field this patch sets replaced (`{ ...settings, ...patch }`).
    public func applied(to settings: Settings) -> Settings {
        var s = settings
        if let typingSpeedWPM { s.typingSpeedWPM = typingSpeedWPM }
        if let defaultTriggerPrefix { s.defaultTriggerPrefix = defaultTriggerPrefix }
        if let paletteHotkey { s.paletteHotkey = paletteHotkey }
        if let paused { s.paused = paused }
        if let defaultInsertion { s.defaultInsertion = defaultInsertion }
        if let deniedBundleIDs { s.deniedBundleIDs = deniedBundleIDs }
        if let appInsertions { s.appInsertions = appInsertions }
        if let launchAtLogin { s.launchAtLogin = launchAtLogin }
        if let onboardingCompleted { s.onboardingCompleted = onboardingCompleted }
        if let theme { s.theme = theme }
        if let showInDock { s.showInDock = showInDock }
        if let autoCheckUpdates { s.autoCheckUpdates = autoCheckUpdates }
        if let updateChannel { s.updateChannel = updateChannel }
        if let icloudSync { s.icloudSync = icloudSync }
        if let lastRunVersion { s.lastRunVersion = lastRunVersion }
        if let clipboardEnabled { s.clipboardEnabled = clipboardEnabled }
        if let clipboardRetentionDays { s.clipboardRetentionDays = clipboardRetentionDays }
        if let clipboardHotkey { s.clipboardHotkey = clipboardHotkey }
        if let clipboardIgnoredApps { s.clipboardIgnoredApps = clipboardIgnoredApps }
        if let clipboardPastePlain { s.clipboardPastePlain = clipboardPastePlain }
        if let clipboardShowSecrets { s.clipboardShowSecrets = clipboardShowSecrets }
        if let agentAccess { s.agentAccess = agentAccess }
        if let agentWrite { s.agentWrite = agentWrite }
        if let agentClipboard { s.agentClipboard = agentClipboard }
        return s
    }
}
