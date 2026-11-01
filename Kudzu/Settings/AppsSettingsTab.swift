import AppKit
import KudzuCore
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
            let na = AppCatalog.info(for: a).name
            let nb = AppCatalog.info(for: b).name
            let cmp = na.localizedStandardCompare(nb)
            if cmp != .orderedSame { return cmp == .orderedAscending }
            return a < b
        }
    }

    var body: some View {
        Form {
            Section("Default Insertion") {
                Picker("Default Insertion Method", selection: $settings.defaultInsertion) {
                    Text(InsertionMethod.automatic.title).tag(InsertionMethod.automatic)
                    Text(InsertionMethod.type.title).tag(InsertionMethod.type)
                    Text(InsertionMethod.paste.title).tag(InsertionMethod.paste)
                }

                Text("Automatic types short plain text and pastes longer text. Snippet, group, and per-app overrides take precedence.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section("Per-App Insertion Overrides") {
                Text("Force Type or Paste in specific apps when neither the snippet nor its group sets an insertion method. Terminals default to Paste.")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                let overrideIDs = sortedOverrideBundleIDs
                if overrideIDs.isEmpty {
                    Text("No per-app insertion overrides.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                } else {
                    VStack(spacing: 0) {
                        ForEach(overrideIDs, id: \.self) { bundleID in
                            AppInsertionOverrideRow(
                                bundleID: bundleID,
                                method: Binding(
                                    get: { settings.appInsertions[bundleID] ?? .paste },
                                    set: { settings.setAppInsertion($0, for: bundleID) }
                                ),
                                remove: { settings.removeAppInsertion(bundleID) }
                            )
                            if bundleID != overrideIDs.last { Divider() }
                        }
                    }
                    .background(.fill.quaternary, in: RoundedRectangle(cornerRadius: 8))
                }

                HStack {
                    AddAppMenu(existing: Set(overrideIDs)) { bundleID in
                        settings.addAppInsertion(bundleID, method: .paste)
                    }
                    Spacer()
                    Button("Restore Defaults") {
                        settings.restoreDefaultAppInsertions()
                    }
                    .disabled(settings.isDefaultAppInsertions)
                }
            }

            Section("Excluded Apps") {
                Text("Kudzu never expands snippets while these apps are active, even if a snippet or group rule allows them.")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                let deniedIDs = settings.deniedBundleIDs
                if deniedIDs.isEmpty {
                    Text("No excluded apps.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                } else {
                    VStack(spacing: 0) {
                        ForEach(deniedIDs, id: \.self) { bundleID in
                            AppRow(bundleID: bundleID) {
                                settings.removeDeniedApp(bundleID)
                            }
                            if bundleID != deniedIDs.last { Divider() }
                        }
                    }
                    .background(.fill.quaternary, in: RoundedRectangle(cornerRadius: 8))
                }

                HStack {
                    AddAppMenu(existing: Set(deniedIDs)) { bundleID in
                        settings.addDeniedApp(bundleID)
                    }
                    Spacer()
                    Button("Restore Defaults") {
                        settings.restoreDefaultDeniedApps()
                    }
                    .disabled(settings.isDefaultDeniedList)
                }
            }
        }
        .formStyle(.grouped)
        .padding()
    }
}

private struct AppInsertionOverrideRow: View {
    var bundleID: String
    @Binding var method: InsertionMethod
    var remove: () -> Void

    var body: some View {
        let info = AppCatalog.info(for: bundleID)
        HStack(spacing: 10) {
            Image(nsImage: info.icon)
                .resizable()
                .frame(width: 24, height: 24)
                .opacity(info.isInstalled ? 1 : 0.5)
            VStack(alignment: .leading, spacing: 0) {
                Text(info.name)
                if info.isInstalled {
                    Text(bundleID).font(.caption).foregroundStyle(.secondary)
                } else {
                    Text("\(bundleID) · Not installed").font(.caption).foregroundStyle(.secondary)
                }
            }
            Spacer()
            Picker("Insertion", selection: $method) {
                Text(InsertionMethod.type.title).tag(InsertionMethod.type)
                Text(InsertionMethod.paste.title).tag(InsertionMethod.paste)
            }
            .labelsHidden()
            .fixedSize()
            Button(action: remove) {
                Image(systemName: "minus.circle.fill")
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .help("Remove \(info.name)")
            .accessibilityLabel("Remove \(info.name)")
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
    }
}
