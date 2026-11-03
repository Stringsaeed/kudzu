import AppKit
import KudzuCore
import KudzuUI
import SwiftUI
import Testing

/// Native offscreen artifacts for visual review; these are fixtures, not live-app verification.
@MainActor
struct DesignCaptureTests {
    @Test func renderDesignFixtures() async throws {
        NSApplication.shared.setActivationPolicy(.prohibited)
        KzFonts.register()
        let directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .appending(path: "build/design-review", directoryHint: .isDirectory)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        for dark in [false, true] {
            let library = LibraryModel(store: try KudzuStore.inMemory())
            library.ensureLoaded()
            DebugSeed.demo(library: library)
            let group = GroupRecord(id: DebugSeed.id(70), name: "Support")
            try library.store.upsert(group)
            let snippet = SnippetRecord(id: DebugSeed.id(90), name: "First response", triggers: [";reply", ";followup"], content: """
            Hi {{field name="first_name" default="Alex"}},

            Thanks for getting in touch. I've picked up your request and will help you work through it.

            Here's what happens next:
            • I'll review the details you've shared.
            • If I need anything else, I'll reply here.
            • You'll hear back from me by {{date offset="+1d" format="EEEE"}}.

            You can reply to this message at any time.

            Best,
            Saeed
            {{cursor}}
            """, groupID: group.id, isFavorite: true)
            try library.store.upsert(snippet)
            library.reload()
            let browser = BrowserState()
            browser.selection = [snippet.id]
            let appearance = dark ? "dark" : "light"
            let suite = UserDefaults(suiteName: "sh.saeed.kudzu.design-capture.\(UUID().uuidString)")!
            let settings = AppMatchingSettings(defaults: suite)
            let workspace = DesignCaptureWorkspace(settings: settings, mode: .library)
                .environment(library).environment(browser).defaultAppStorage(suite)
            try await render(workspace, size: CGSize(width: 1180, height: 760), dark: dark, windowChrome: true,
                             to: directory.appending(path: "library-\(appearance).png"))
            try await render(workspace, size: CGSize(width: 862, height: 600), dark: dark, windowChrome: true,
                             to: directory.appending(path: "library-compact-\(appearance).png"))
            suite.set(true, forKey: "editor.previewExpanded")
            try await render(workspace, size: CGSize(width: 1180, height: 760), dark: dark, windowChrome: true,
                             to: directory.appending(path: "library-preview-\(appearance).png"))
            suite.set(false, forKey: "editor.previewExpanded")
            browser.selection = []
            try await render(workspace, size: CGSize(width: 1180, height: 760), dark: dark, windowChrome: true,
                             to: directory.appending(path: "library-empty-\(appearance).png"))
            browser.selection = [snippet.id]
            let modal = DesignCaptureWorkspace(settings: settings, mode: .settings(.apps))
                .environment(library).environment(browser).defaultAppStorage(suite)
            try await render(modal, size: CGSize(width: 1180, height: 760), dark: dark, windowChrome: true,
                             to: directory.appending(path: "settings-\(appearance).png"))
            let palette = SearchPaletteModel(store: library.store)
            palette.prepareForShow()
            try await render(SearchPaletteView(model: palette), size: SearchPaletteController.panelSize, dark: dark,
                             to: directory.appending(path: "palette-\(appearance).png"))
            palette.query = "nothing-matches-this"
            try await render(SearchPaletteView(model: palette), size: SearchPaletteController.panelSize, dark: dark,
                             to: directory.appending(path: "palette-empty-\(appearance).png"))
        }
    }

    private func render<V: View>(_ view: V, size: CGSize, dark: Bool, windowChrome: Bool = false, to url: URL) async throws {
        let host = NSHostingView(rootView: view.environment(\.colorScheme, dark ? .dark : .light))
        host.sizingOptions = []
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size),
                              styleMask: windowChrome ? [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView] : [.borderless],
                              backing: .buffered, defer: false)
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        window.contentView = host
        defer { window.contentView = nil }
        host.frame = NSRect(origin: .zero, size: size)
        host.layoutSubtreeIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
        host.layoutSubtreeIfNeeded()
        let drawingView = windowChrome ? (host.superview ?? host) : host
        let bitmap = try #require(drawingView.bitmapImageRepForCachingDisplay(in: drawingView.bounds))
        window.appearance?.performAsCurrentDrawingAppearance {
            drawingView.cacheDisplay(in: drawingView.bounds, to: bitmap)
        }
        let png = try #require(bitmap.representation(using: .png, properties: [:]))
        try png.write(to: url, options: .atomic)
        #expect(bitmap.pixelsWide > 0 && bitmap.pixelsHigh > 0)
    }
}

private struct DesignCaptureWorkspace: View {
    let settings: AppMatchingSettings
    let mode: MainWindowMode
    @FocusState private var searchFocused: Bool

    var body: some View {
        MainWindowShell(shellMode: .window, windowMode: .constant(mode), lastSettingsSection: .apps,
                        storeError: nil, searchFocus: $searchFocused) { _ in
            AppsSettingsTab(settings: settings)
        }
    }
}
