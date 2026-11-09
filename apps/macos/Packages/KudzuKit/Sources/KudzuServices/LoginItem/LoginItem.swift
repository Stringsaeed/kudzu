import AppKit
import Foundation
import ServiceManagement

/// Launch at login (tauri-plugin-autostart in the Tauri app) through `SMAppService.mainApp`.
///
/// The Tauri app registered a LaunchAgent (`~/Library/LaunchAgents/Kudzu.plist`) that launched
/// its `kudzu-desktop` executable with `--hidden`. A login item registered with `SMAppService`
/// can't pass arguments, so a login launch is recognised from the launch Apple event instead
/// (``launchedAsLoginItem()``). The first launch after the Tauri app replaces that agent with the
/// login item (``migrateLegacyLaunchAgent(productName:bundleID:)``), and changing the setting
/// removes any agent that is left, so the two never both launch Kudzu.
public enum LoginItem {
    private static let log = Logging.logger(category: "login-item")

    /// Turns launch at login on or off.
    public static func set(enabled: Bool) throws(ServiceError) {
        let service = SMAppService.mainApp
        do {
            if enabled {
                if service.status != .enabled { try service.register() }
            } else if service.status == .enabled || service.status == .requiresApproval {
                try service.unregister()
            }
            removeLegacyLaunchAgent()
        } catch {
            log.warning("autostart: \(error.localizedDescription)")
            throw ServiceError(error.localizedDescription)
        }
    }

    /// Whether the login item is registered (it may still await approval in System Settings).
    public static var isEnabled: Bool {
        let s = SMAppService.mainApp.status
        return s == .enabled || s == .requiresApproval
    }

    /// True when this launch came from the login item. Call from
    /// `applicationWillFinishLaunching`/`applicationDidFinishLaunching`, while the launch
    /// event is still current.
    @MainActor
    public static func launchedAsLoginItem() -> Bool {
        guard let event = NSAppleEventManager.shared().currentAppleEvent,
              event.eventID == AEEventID(kAEOpenApplication),
              let prop = event.paramDescriptor(forKeyword: AEKeyword(keyAEPropData)) else { return false }
        return prop.enumCodeValue == OSType(keyAELaunchedAsLogInItem)
    }

    /// The Tauri app's agent existed only while its launch-at-login setting was on. Replace it
    /// with the login item, so `isEnabled` matches the setting and login no longer starts the
    /// old executable. Returns true when an agent was migrated. Call once at launch.
    @discardableResult
    public static func migrateLegacyLaunchAgent(productName: String, bundleID: String) -> Bool {
        guard let url = legacyAgentURL(productName: productName, bundleID: bundleID) else { return false }
        let service = SMAppService.mainApp
        do {
            try migrateLegacyAgent(at: url) {
                if service.status != .enabled { try service.register() }
            }
            log.info("launch at login moved from the previous login agent to the login item")
        } catch {
            log.warning("autostart: couldn't register the login item: \(error.localizedDescription)")
            return false
        }
        return true
    }

    /// Removes the Tauri app's LaunchAgent for this variant. True when one was removed.
    @discardableResult
    static func removeLegacyLaunchAgent(productName: String = Bundle.main.object(forInfoDictionaryKey: "CFBundleName") as? String ?? "Kudzu", bundleID: String = Bundle.main.bundleIdentifier ?? "") -> Bool {
        guard let url = legacyAgentURL(productName: productName, bundleID: bundleID) else { return false }
        do {
            try FileManager.default.removeItem(at: url)
            log.info("removed the previous login agent \(url.lastPathComponent)")
            return true
        } catch {
            log.warning("couldn't remove the previous login agent: \(error.localizedDescription)")
            return false
        }
    }

    static func migrateLegacyAgent(at url: URL, register: () throws -> Void) throws {
        try register()
        try FileManager.default.removeItem(at: url)
    }

    private static func legacyAgentURL(productName: String, bundleID: String) -> URL? {
        let url = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/LaunchAgents", isDirectory: true)
            .appendingPathComponent("\(productName).plist")
        guard let data = try? Data(contentsOf: url),
              let plist = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any],
              isLegacyAgent(plist, label: productName, bundleID: bundleID) else { return nil }
        return url
    }

    /// Whether a LaunchAgent plist is the one tauri-plugin-autostart (auto-launch) wrote for
    /// Kudzu: matched by label plus what it launches (the Tauri executable, this bundle id, or
    /// this bundle). Not by this bundle's path alone: the native app may live elsewhere.
    static func isLegacyAgent(_ plist: [String: Any], label: String, bundleID: String) -> Bool {
        guard (plist["Label"] as? String) == label,
              let program = (plist["ProgramArguments"] as? [String])?.first else { return false }
        if (plist["AssociatedBundleIdentifiers"] as? [String])?.contains(bundleID) == true { return true }
        return URL(fileURLWithPath: program).lastPathComponent == "kudzu-desktop"
            || program.hasPrefix(Bundle.main.bundleURL.path + "/")
    }
}
