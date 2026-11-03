import AppKit
import KudzuCore
import KudzuUI
import SwiftUI

/// JSON-serializable global app exclusions and insertion settings (ready for M4 CloudKit sync).
struct GlobalAppSettings: Codable, Hashable, Sendable {
    var defaultInsertion: InsertionMethod
    var deniedBundleIDs: [String]
    var appInsertions: [String: InsertionMethod]

    init(
        defaultInsertion: InsertionMethod = .automatic,
        deniedBundleIDs: [String] = MatchingConfig.defaultDeniedBundleIDs.sorted(),
        appInsertions: [String: InsertionMethod] = MatchingConfig.defaultAppInsertions
    ) {
        self.defaultInsertion = defaultInsertion
        self.deniedBundleIDs = deniedBundleIDs
        self.appInsertions = appInsertions
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        defaultInsertion = try container.decodeIfPresent(InsertionMethod.self, forKey: .defaultInsertion) ?? .automatic
        deniedBundleIDs = try container.decodeIfPresent([String].self, forKey: .deniedBundleIDs)
            ?? MatchingConfig.defaultDeniedBundleIDs.sorted()
        appInsertions = try container.decodeIfPresent([String: InsertionMethod].self, forKey: .appInsertions)
            ?? MatchingConfig.defaultAppInsertions
    }

    var matchingConfig: MatchingConfig {
        MatchingConfig(
            defaultInsertion: defaultInsertion,
            deniedBundleIDs: Set(deniedBundleIDs),
            appInsertions: appInsertions
        )
    }
}

/// Persists `GlobalAppSettings` in `UserDefaults` and produces a `MatchingConfig` for the engine.
@MainActor @Observable
final class AppMatchingSettings {
    static let defaultsKey = "globalAppSettings"
    static let didChangeNotification = Notification.Name("sh.saeed.kudzu.appMatchingSettingsDidChange")
    static let shared = AppMatchingSettings()

    @ObservationIgnored private let defaults: UserDefaults
    private(set) var value: GlobalAppSettings

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        if let data = defaults.data(forKey: Self.defaultsKey),
           let decoded = try? JSONDecoder().decode(GlobalAppSettings.self, from: data) {
            self.value = decoded
        } else {
            self.value = GlobalAppSettings()
        }
    }

    var matchingConfig: MatchingConfig {
        value.matchingConfig
    }

    var defaultInsertion: InsertionMethod {
        get { value.defaultInsertion }
        set {
            guard value.defaultInsertion != newValue else { return }
            value.defaultInsertion = newValue
            save()
        }
    }

    var deniedBundleIDs: [String] {
        value.deniedBundleIDs
    }

    var isDefaultDeniedList: Bool {
        Set(value.deniedBundleIDs) == MatchingConfig.defaultDeniedBundleIDs
    }

    func addDeniedApp(_ bundleID: String) {
        guard !value.deniedBundleIDs.contains(bundleID) else { return }
        value.deniedBundleIDs.append(bundleID)
        save()
    }

    func removeDeniedApp(_ bundleID: String) {
        let countBefore = value.deniedBundleIDs.count
        value.deniedBundleIDs.removeAll { $0 == bundleID }
        guard value.deniedBundleIDs.count != countBefore else { return }
        save()
    }

    func restoreDefaultDeniedApps() {
        let restored = MatchingConfig.defaultDeniedBundleIDs.sorted()
        guard value.deniedBundleIDs != restored else { return }
        value.deniedBundleIDs = restored
        save()
    }

    var appInsertions: [String: InsertionMethod] {
        value.appInsertions
    }

    var isDefaultAppInsertions: Bool {
        value.appInsertions == MatchingConfig.defaultAppInsertions
    }

    func addAppInsertion(_ bundleID: String, method: InsertionMethod = .paste) {
        guard value.appInsertions[bundleID] == nil else { return }
        value.appInsertions[bundleID] = method
        save()
    }

    func setAppInsertion(_ method: InsertionMethod, for bundleID: String) {
        guard value.appInsertions[bundleID] != method else { return }
        value.appInsertions[bundleID] = method
        save()
    }

    func removeAppInsertion(_ bundleID: String) {
        guard value.appInsertions.removeValue(forKey: bundleID) != nil else { return }
        save()
    }

    func restoreDefaultAppInsertions() {
        guard value.appInsertions != MatchingConfig.defaultAppInsertions else { return }
        value.appInsertions = MatchingConfig.defaultAppInsertions
        save()
    }

    private func save() {
        if let data = try? JSONEncoder().encode(value) {
            defaults.set(data, forKey: Self.defaultsKey)
        }
        NotificationCenter.default.post(name: Self.didChangeNotification, object: self)
    }
}

/// Settings tab for global default insertion, per-app insertion overrides, and excluded apps.
struct AppsSettingsTab: View {
    @Bindable var settings: AppMatchingSettings = .shared

    private var sortedOverrideBundleIDs: [String] {
        settings.appInsertions.keys.sorted { a, b in
            let na = SettingsAppPresentation.name(for: a)
            let nb = SettingsAppPresentation.name(for: b)
            let cmp = na.localizedStandardCompare(nb)
            if cmp != .orderedSame { return cmp == .orderedAscending }
            return a < b
        }
    }

    var body: some View {
        SettingsPageLayout(title: "Apps") {
            WorkspaceSection("Default insertion") {
                WorkspaceSettingRow(
                    "Insertion method",
                    description: "Automatic types short plain text and pastes longer text. Snippet, group and app overrides take precedence."
                ) {
                    Picker("Default insertion method", selection: $settings.defaultInsertion) {
                        ForEach([InsertionMethod.automatic, .type, .paste], id: \.self) { method in
                            Text(method.title).tag(method)
                        }
                    }
                    .labelsHidden()
                    .frame(width: 140)
                }
            }
            WorkspaceSection(
                "Per-app insertion overrides",
                description: "Choose Type or Paste when neither the snippet nor its group sets a method. Terminals default to Paste."
            ) {
                if sortedOverrideBundleIDs.isEmpty {
                    WorkspaceSettingRow("No per-app insertion overrides") { EmptyView() }
                }
                ForEach(sortedOverrideBundleIDs, id: \.self) { bundleID in
                    SettingsAppRow(bundleID: bundleID) {
                        Picker("Insertion", selection: Binding(
                            get: { settings.appInsertions[bundleID] ?? .paste },
                            set: { settings.setAppInsertion($0, for: bundleID) }
                        )) {
                            ForEach([InsertionMethod.type, .paste], id: \.self) { method in
                                Text(method.title).tag(method)
                            }
                        }
                        .labelsHidden()
                        .frame(width: 100)
                        removeButton(bundleID) { settings.removeAppInsertion(bundleID) }
                    }
                }
                appActions(
                    existing: Set(sortedOverrideBundleIDs),
                    isDefault: settings.isDefaultAppInsertions,
                    add: { settings.addAppInsertion($0, method: .paste) },
                    restore: settings.restoreDefaultAppInsertions
                )
            }
            WorkspaceSection(
                "Excluded apps",
                description: "Kudzu never expands snippets in these apps, even when a snippet or group rule allows them."
            ) {
                if settings.deniedBundleIDs.isEmpty {
                    WorkspaceSettingRow("No excluded apps") { EmptyView() }
                }
                ForEach(settings.deniedBundleIDs, id: \.self) { bundleID in
                    SettingsAppRow(bundleID: bundleID) {
                        removeButton(bundleID) { settings.removeDeniedApp(bundleID) }
                    }
                }
                appActions(
                    existing: Set(settings.deniedBundleIDs),
                    isDefault: settings.isDefaultDeniedList,
                    add: settings.addDeniedApp,
                    restore: settings.restoreDefaultDeniedApps
                )
            }
        }
    }

    private func removeButton(_ bundleID: String, action: @escaping () -> Void) -> some View {
        Button("Remove", action: action)
            .controlSize(.small)
            .buttonStyle(.borderless)
            .tint(WorkspaceTheme.secondary)
            .accessibilityLabel("Remove \(AppCatalog.info(for: bundleID).name)")
    }

    private func appActions(
        existing: Set<String>,
        isDefault: Bool,
        add: @escaping (String) -> Void,
        restore: @escaping () -> Void
    ) -> some View {
        HStack {
            Menu {
                ForEach(AppCatalog.runningApps(excluding: existing), id: \.bundleID) { app in
                    Button(app.name) { add(app.bundleID) }
                }
                Divider()
                Button("Choose app…") {
                    if let id = AppCatalog.chooseApp() { add(id) }
                }
            } label: {
                Text("Add app…")
            }
            .controlSize(.small)
            .menuStyle(.borderlessButton)
            Spacer()
            Button("Restore defaults", action: restore)
                .controlSize(.small)
                .buttonStyle(.borderless)
                .disabled(isDefault)
        }
        .padding(.vertical, 10)
    }
}

private struct SettingsAppRow<Controls: View>: View {
    let bundleID: String
    @ViewBuilder var controls: Controls

    var body: some View {
        let info = AppCatalog.info(for: bundleID)
        HStack(spacing: 10) {
            Image(nsImage: info.icon).resizable().frame(width: 22, height: 22)
                .opacity(info.isInstalled ? 1 : 0.5)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(SettingsAppPresentation.name(for: bundleID))
                    .font(WorkspaceTheme.body)
                    .foregroundStyle(WorkspaceTheme.ink)
                if !info.isInstalled {
                    Text("Not installed on this Mac")
                        .font(WorkspaceTheme.caption)
                        .foregroundStyle(WorkspaceTheme.secondary)
                }
            }
            Spacer(minLength: 8)
            controls
        }
        .padding(.vertical, 10)
        .help(bundleID)
        .overlay(alignment: .bottom) { WorkspaceHairline() }
    }
}

@MainActor
private enum SettingsAppPresentation {
    static func name(for bundleID: String) -> String {
        let info = AppCatalog.info(for: bundleID)
        guard !info.isInstalled else { return info.name }
        switch bundleID {
        case "com.github.wez.wezterm": return "WezTerm"
        case "com.googlecode.iterm2": return "iTerm2"
        case "dev.warp.Warp-Stable": return "Warp"
        case "net.kovidgoyal.kitty": return "kitty"
        case "org.alacritty": return "Alacritty"
        case "com.1password.1password": return "1Password"
        case "com.agilebits.onepassword7": return "1Password 7"
        case "com.agilebits.onepassword-osx": return "1Password (legacy)"
        case "com.bitwarden.desktop": return "Bitwarden"
        case "com.apple.keychainaccess": return "Keychain Access"
        case "com.apple.Passwords": return "Passwords"
        case "com.apple.loginwindow": return "Login window"
        default: return info.name
        }
    }
}
