import Foundation
import KudzuCore

/// OS services that don't need a running engine. Port of `lib.rs` `pub mod system` and
/// `macos/mod.rs`.
public enum System {
    /// Current permission grants (cheap; safe to poll).
    public static func permissions() -> PermissionStatus {
        AX.permissions()
    }

    /// Shows the system Accessibility prompt.
    public static func requestAccessibility() {
        AX.requestAccessibility()
    }

    /// Shows the system Input Monitoring prompt.
    public static func requestInputMonitoring() {
        AX.requestInputMonitoring()
    }

    /// Opens System Settings at `pane`: "accessibility" | "inputMonitoring" | "loginItems".
    public static func openSettings(pane: String) {
        let url = switch pane {
        case "accessibility": "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        case "inputMonitoring": "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
        case "loginItems": "x-apple.systempreferences:com.apple.LoginItems-Settings.extension"
        default: "x-apple.systempreferences:com.apple.preference.security"
        }
        if !Apps.openURL(url) {
            EngineLog.logger.warning("could not open System Settings pane \(pane, privacy: .public)")
        }
    }

    /// Opens System Settings at `pane`.
    public static func openSettings(_ pane: SystemSettingsPane) {
        openSettings(pane: pane.rawValue)
    }

    /// The frontmost application.
    public static func frontmostApp() -> AppInfo? {
        Apps.frontmostApp()
    }

    /// Brings `app` (by pid, else bundle id) to the front.
    @discardableResult
    public static func activateApp(_ app: AppInfo) -> Bool {
        Apps.activate(app)
    }

    /// Installed apps (/Applications, ~/Applications, /System/Applications, …), sorted by name.
    /// Reads every bundle's Info.plist: call off the UI thread.
    public static func installedApps() -> [AppInfo] {
        Apps.installedApps()
    }

    /// PNG bytes of an app's icon at `size` px, by bundle path.
    public static func appIconPNG(path: String, size: Int) -> Data? {
        Apps.iconPNG(path: path, size: size)
    }

    /// Caret rectangle of the focused text element via Accessibility.
    public static func caretRect() -> ScreenRect? {
        AX.caretRect()
    }

    /// Copies the current selection in the frontmost app (⌘C, read, restore clipboard).
    /// Blocks up to ~300 ms: call off the UI thread.
    public static func copySelection() -> String? {
        AX.selectedText() ?? Pasteboard.copySelection(timeout: 0.3)
    }

    /// Plain-text clipboard.
    public static func readClipboard() -> String? {
        Pasteboard.readString()
    }

    /// Writes plain text to the clipboard.
    @discardableResult
    public static func writeClipboard(_ text: String) -> Bool {
        Pasteboard.writeString(text)
    }

    /// Whether Secure Input is on, and the owning app's name if known.
    public static func secureInput() -> (on: Bool, owner: String?) {
        let (on, pid) = AX.secureInput()
        return (on, pid.flatMap(Apps.name(of:)))
    }

    /// Best-effort system locale identifier, e.g. `en_US`.
    public static func locale() -> String {
        // Drop keyword suffixes such as `@rg=gbzzzz`.
        let identifier = Locale.current.identifier
        let base = (identifier.split(separator: "@", omittingEmptySubsequences: false).first.map(String.init) ?? "")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return base.isEmpty ? "en_US" : base
    }
}
