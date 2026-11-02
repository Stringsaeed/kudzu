import AppKit
import KudzuUI
import SwiftUI

/// `KudzuGallery --window-diagnostics`: opens a Kudzu-style window (`KzAppShell(.window)`), prints what
/// AppKit built in its titlebar area, and quits. For debugging window chrome without a screenshot.
struct WindowDiagnosticsApp: App {
    var body: some Scene {
        Window("Kudzu", id: "main") {
            KzAppShell(mode: .window) {
                KzSidebarSection("Library") { Text("All snippets") }
            } sidebarFooter: {
                Text("Settings")
            } list: {
                KzListHeader("All snippets", subtitle: "18 snippets")
                Spacer()
            } detail: {
                KzPaneHeader { Text("base") }
                Spacer()
            }
            .task {
                if CommandLine.arguments.contains("--as-menu-bar-app") {
                    // Mirror Kudzu: an accessory app that turns regular when its window opens.
                    NSApp.setActivationPolicy(.regular)
                    NSApp.activate()
                    NSApp.windows.first?.makeKeyAndOrderFront(nil)
                }
                try? await Task.sleep(for: .seconds(1.5))
                WindowDiagnostics.dump()
                NSApp.terminate(nil)
            }
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1080, height: 700)
    }
}

@MainActor
enum WindowDiagnostics {
    static func dump() {
        guard let window = NSApp.windows.first(where: { $0.contentView is NSHostingView<AnyView> || $0.title == "Kudzu" }) ?? NSApp.windows.first else {
            print("no window"); return
        }
        print("window frame", window.frame, "contentLayoutRect", window.contentLayoutRect)
        print("toolbar", window.toolbar.map { "\($0.identifier) items=\($0.items.count) visible=\($0.isVisible)" } ?? "nil",
              "toolbarStyle", window.toolbarStyle.rawValue)
        print("titlebarAppearsTransparent", window.titlebarAppearsTransparent, "titleVisibility", window.titleVisibility.rawValue,
              "fullSize", window.styleMask.contains(.fullSizeContentView), "opaque", window.isOpaque)
        if let content = window.contentView {
            print("contentView", type(of: content), content.frame, "safeArea", content.safeAreaInsets)
        }
        if let close = window.standardWindowButton(.closeButton) {
            print("close button in window", close.convert(close.bounds, to: nil))
        }
        guard let frameView = window.contentView?.superview else { return }
        let top = window.frame.height
        func walk(_ view: NSView, depth: Int) {
            let rect = view.convert(view.bounds, to: nil)
            // Only views that reach into the top 60pt of the window.
            guard rect.maxY > top - 60 || depth < 2 else { return }
            let pad = String(repeating: "  ", count: depth)
            var extra = ""
            if let effect = view as? NSVisualEffectView { extra += " material=\(effect.material.rawValue) blending=\(effect.blendingMode.rawValue) state=\(effect.state.rawValue)" }
            if view.isHidden { extra += " HIDDEN" }
            if view.alphaValue < 1 { extra += " alpha=\(view.alphaValue)" }
            if let layer = view.layer, let bg = layer.backgroundColor { extra += " layerBG=\(bg)" }
            print("\(pad)\(type(of: view)) \(NSStringFromRect(rect))\(extra)")
            if depth < 7 { for sub in view.subviews { walk(sub, depth: depth + 1) } }
        }
        walk(frameView, depth: 0)
    }
}
