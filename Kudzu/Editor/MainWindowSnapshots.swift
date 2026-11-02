#if DEBUG
import AppKit
import KudzuCore
import KudzuUI
import SwiftUI

/// DEBUG builds only: `Kudzu -KudzuSnapshot <dir>` renders the main window offscreen, embedded, from
/// the demo library in an in-memory store, writes Light and Dark PNGs into `dir`, and exits. It runs
/// before `AppModel` exists, so nothing starts: no event tap, no shortcut, no window.
@MainActor
enum MainWindowSnapshots {
    static let size = CGSize(width: 1180, height: 760)

    /// Runs and exits when the launch arguments ask for it; returns otherwise.
    static func runIfRequested() {
        let arguments = CommandLine.arguments
        guard let index = arguments.firstIndex(of: "-KudzuSnapshot"), index + 1 < arguments.count else { return }
        let directory = URL(filePath: arguments[index + 1], relativeTo: URL(filePath: FileManager.default.currentDirectoryPath))
        do {
            try run(to: directory)
            exit(0)
        } catch {
            FileHandle.standardError.write(Data("Snapshot failed: \(error)\n".utf8))
            exit(1)
        }
    }

    private static func run(to directory: URL) throws {
        let library = LibraryModel(store: try KudzuStore.inMemory())
        library.ensureLoaded()
        DebugSeed.demo(library: library)
        // Its own defaults, so the snapshots never read or change the user's preferences.
        let suite = "sh.saeed.kudzu.snapshots"
        let defaults = UserDefaults(suiteName: suite) ?? .standard
        defaults.removePersistentDomain(forName: suite)

        let signature = DebugSeed.id(10)
        let meeting = DebugSeed.id(13)
        let address = DebugSeed.id(14)
        let shots: [(String, SidebarScope, Set<UUID>, MainWindowMode, (rules: Bool, advanced: Bool))] = [
            ("window-all", .all, [signature], .library, (false, false)),
            ("window-rules", .all, [address], .library, (true, false)),
            ("window-advanced", .all, [meeting], .library, (false, true)),
            ("window-group", .group(DebugSeed.id(1)), [meeting], .library, (false, false)),
            ("window-empty", .all, [], .library, (false, false)),
            ("window-multi", .all, [signature, meeting], .library, (false, false)),
            ("window-stats", .stats, [], .library, (false, false)),
            ("window-settings", .all, [], .settings(.general), (false, false)),
        ]
        for (name, scope, selection, mode, expanded) in shots {
            defaults.set(expanded.rules, forKey: "editor.rulesExpanded")
            defaults.set(expanded.advanced, forKey: "editor.advancedExpanded")
            let browser = BrowserState()
            browser.scope = scope
            browser.selection = selection
            let view = SnapshotWindow(mode: mode)
                .environment(library)
                .environment(browser)
                .defaultAppStorage(defaults)
                .frame(width: size.width, height: size.height)
                .padding(KzSpace.s6)
            try KzSnapshot.write(view, named: name, to: directory, width: size.width + 2 * KzSpace.s6, background: .canvas)
        }
        defaults.removePersistentDomain(forName: suite)
    }
}

/// Owns the focus state the shell needs.
private struct SnapshotWindow: View {
    let mode: MainWindowMode
    @FocusState private var searchFocused: Bool

    var body: some View {
        MainWindowShell(
            shellMode: .embedded,
            windowMode: .constant(mode),
            lastSettingsSection: .general,
            storeError: nil,
            searchFocus: $searchFocused
        ) { section in
            // Settings pages need the whole `AppModel`; snapshots only show where the page goes.
            VStack(alignment: .leading, spacing: KzSpace.s1) {
                Text(section.title).kzTextStyle(.titleLg).foregroundStyle(KzColor.ink)
                Text("SettingsPage(section: .\(section.rawValue)) is hosted here, scrolling, on surface.")
                    .kzTextStyle(.body)
                    .foregroundStyle(KzColor.inkMuted)
            }
            .padding(.top, KzSpace.s5)
        }
    }
}
#endif
