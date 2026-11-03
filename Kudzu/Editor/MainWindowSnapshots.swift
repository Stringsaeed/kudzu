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
        let directory = URL(filePath: arguments[index + 1], relativeTo: URL(fileURLWithPath: FileManager.default.currentDirectoryPath, isDirectory: true))
        do {
            try run(to: directory)
            exit(0)
        } catch {
            FileHandle.standardError.write(Data("Snapshot failed: \(error)\n".utf8))
            exit(1)
        }
    }

    private static func run(to directory: URL) throws {
        let signature = DebugSeed.id(10)
        let meeting = DebugSeed.id(13)
        let address = DebugSeed.id(14)
        let shots: [(String, SidebarScope, Set<UUID>, MainWindowMode, (rules: Bool, advanced: Bool))] = [
            ("window-all", .all, [signature], .library, (false, false)),
            ("window-edited", .all, [signature], .library, (false, false)),
            ("window-rules", .all, [address], .library, (true, false)),
            ("window-advanced", .all, [meeting], .library, (false, true)),
            ("window-group", .group(DebugSeed.id(1)), [meeting], .library, (false, false)),
            ("window-empty", .all, [], .library, (false, false)),
            ("window-multi", .all, [signature, meeting], .library, (false, false)),
            ("window-stats", .stats, [], .library, (false, false)),
            ("window-settings", .all, [], .settings(.general), (false, false)),
        ]
        let sizes = [("", size), ("-compact", CGSize(width: 900, height: 600)),
                     ("-minimum", CGSize(width: 862, height: 480))]
        for (suffix, size) in sizes {
            for (name, scope, selection, mode, expanded) in shots {
                for appearance in KzSnapshot.Appearance.allCases {
                    // Rendering tears the editor down and saves its draft. Each image needs its own store.
                    let library = LibraryModel(store: try KudzuStore.inMemory())
                    library.ensureLoaded()
                    DebugSeed.demo(library: library)
                    let defaults = UserDefaults(suiteName: "sh.saeed.kudzu.snapshots.\(UUID().uuidString)")!
                    defaults.register(defaults: [
                        "editor.previewExpanded": name == "window-group",
                    ])
                    let browser = BrowserState()
                    browser.scope = scope
                    browser.selection = selection
                    if name == "window-edited", let snippet = library.snippetsByID[signature] {
                        library.editor(for: snippet).edit { $0.name = "Work signature" }
                    }
                    let view = SnapshotWindow(mode: mode)
                        .environment(library)
                        .environment(browser)
                        .environment(\.workspaceInspectorSnapshot, expanded.rules || expanded.advanced)
                        .defaultAppStorage(defaults)
                        .frame(width: size.width, height: size.height)
                        .padding(KzSpace.s6)
                    guard let image = KzSnapshot.image(of: view, appearance: appearance,
                                                      width: size.width + 2 * KzSpace.s6, background: .canvas),
                          let data = KzSnapshot.pngData(image) else {
                        throw CocoaError(.fileWriteUnknown)
                    }
                    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                    try data.write(to: directory.appendingPathComponent("\(name)\(suffix)-\(appearance.rawValue)@2x.png"))
                }
            }
        }
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
