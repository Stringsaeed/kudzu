import AppKit
import UniformTypeIdentifiers

/// Looks up names and icons for bundle IDs, and offers apps to add to an app rule.
@MainActor
enum AppCatalog {
    struct Info {
        var name: String
        var icon: NSImage
        var isInstalled: Bool
    }

    struct RunningApp: Identifiable {
        var bundleID: String
        var name: String
        var id: String { bundleID }
    }

    private static var cache: [String: Info] = [:]

    /// Display name and icon for a bundle ID; falls back to the ID itself when the app isn't installed.
    static func info(for bundleID: String) -> Info {
        if let cached = cache[bundleID] { return cached }
        let info: Info
        if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) {
            info = Info(name: displayName(at: url), icon: NSWorkspace.shared.icon(forFile: url.path), isInstalled: true)
        } else {
            info = Info(name: bundleID, icon: NSWorkspace.shared.icon(for: .applicationBundle), isInstalled: false)
        }
        cache[bundleID] = info
        return info
    }

    private static func displayName(at url: URL) -> String {
        let bundle = Bundle(url: url)
        return bundle?.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String
            ?? bundle?.object(forInfoDictionaryKey: "CFBundleName") as? String
            ?? url.deletingPathExtension().lastPathComponent
    }

    /// Regular (Dock-visible) running apps, excluding `excluding` and Kudzu itself, sorted by name.
    static func runningApps(excluding: Set<String>) -> [RunningApp] {
        let own = Bundle.main.bundleIdentifier
        var seen = Set<String>()
        return NSWorkspace.shared.runningApplications
            .compactMap { app -> RunningApp? in
                guard app.activationPolicy == .regular, let id = app.bundleIdentifier,
                      id != own, !excluding.contains(id), seen.insert(id).inserted else { return nil }
                return RunningApp(bundleID: id, name: app.localizedName ?? info(for: id).name)
            }
            .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }

    /// Shows an open panel in /Applications and returns the chosen app's bundle ID.
    static func chooseApp() -> String? {
        let panel = NSOpenPanel()
        panel.title = "Choose App"
        panel.prompt = "Choose"
        panel.allowedContentTypes = [.applicationBundle]
        panel.allowsMultipleSelection = false
        panel.directoryURL = URL(filePath: "/Applications")
        guard panel.runModal() == .OK, let url = panel.url else { return nil }
        return Bundle(url: url)?.bundleIdentifier
    }
}
