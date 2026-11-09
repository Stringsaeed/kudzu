// Installed apps, loaded once per process (`useInstalledApps` / `appName` in `lib/apps.ts`).

import KudzuCore
import Observation

@MainActor @Observable
public final class InstalledApps {
    /// The shared list; empty until the backend answers (or when it fails, like the web hook).
    public private(set) var apps: [AppInfo] = []
    @ObservationIgnored private var loading: Task<Void, Never>?

    public static let shared = InstalledApps()

    public init() {}

    /// Loads the list once (later calls are no-ops). Running apps come first; otherwise the
    /// backend's order is kept.
    public func load(from backend: any Backend) {
        guard loading == nil else { return }
        loading = Task { [weak self] in
            let list = (try? await backend.listApps()) ?? []
            self?.apps = Self.ordered(list)
        }
    }

    /// Forgets the list so the next `load` asks again (tests, previews).
    public func reset() {
        loading?.cancel()
        loading = nil
        apps = []
    }

    static func ordered(_ apps: [AppInfo]) -> [AppInfo] {
        apps.filter { $0.pid != nil } + apps.filter { $0.pid == nil }
    }

    /// The app's display name, or the bundle id when it isn't installed.
    public static func appName(_ apps: [AppInfo], _ bundleId: String) -> String {
        apps.first { $0.bundleId == bundleId }?.name ?? bundleId
    }
}
