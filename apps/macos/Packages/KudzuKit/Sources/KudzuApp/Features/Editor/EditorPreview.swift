import AppKit
import KudzuCore
import KudzuDesign
import SwiftUI

/// Preview fixtures stay on the in-memory backend and never touch the user's library.
public struct EditorPreview: View {
    @State private var model = AppModel(backend: MockBackend(), isDev: true)
    var empty: Bool
    var formatted: Bool
    public init(empty: Bool = false, formatted: Bool = false) { self.empty = empty; self.formatted = formatted }
    public var body: some View {
        SnippetEditorPane().environment(model)
            .task { await Self.prepare(model, empty: empty, formatted: formatted) }
    }

    @MainActor static func prepare(_ model: AppModel, empty: Bool, formatted: Bool) async {
        await model.start()
        if empty { model.selectedId = nil }
        else if formatted, var snippet = model.library.snippets.first {
            snippet.name = "Welcome message"
            snippet.content = "Welcome, {{field name=Name default=friend}}!\nToday is {{date format=\"MMMM d, yyyy\"}}.\nBest regards,\nThe Kudzu team"
            snippet.formatting = Formatting(source: snippet.content, spans: [FormatSpan(start: 0, end: 7, style: .bold)])
            snippet.rule = AppRule(mode: .allow, bundleIDs: ["com.apple.mail"], insertion: .paste)
            try? await model.saveSnippet(snippet)
            model.selectedId = snippet.id
        }
        await model.editorSession.select(model.selectedId)
        try? await model.editorSession.refreshPreview()
    }
}

#Preview("Editor — light") { EditorPreview().frame(width: 700, height: 1000).preferredColorScheme(.light) }
#Preview("Editor — dark") { EditorPreview().frame(width: 700, height: 1000).preferredColorScheme(.dark) }
#Preview("Editor — formatted") { EditorPreview(formatted: true).frame(width: 700, height: 1000).preferredColorScheme(.dark) }
#Preview("Editor — empty") { EditorPreview(empty: true).frame(width: 700, height: 760) }

/// Explicit render utility for review artifacts. ImageRenderer cannot draw NSViewRepresentable;
/// the companion AppKit capture includes the real NSTextView and native segmented controls.
@MainActor public enum EditorPreviewRenderer {
    public static func render(to directory: URL) async throws {
        let previousAppearance = NSApplication.shared.appearance
        defer { NSApplication.shared.appearance = previousAppearance }
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        for (name, empty, formatted) in [("plain", false, false), ("formatted", false, true), ("empty", true, false)] {
            for dark in [false, true] {
                NSApplication.shared.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
                let model = AppModel(backend: MockBackend(), isDev: true)
                await EditorPreview.prepare(model, empty: empty, formatted: formatted)
                let scheme: ColorScheme = dark ? .dark : .light
                let height: CGFloat = empty ? 760 : 1100
                let root = SnippetEditorPane().environment(model).preferredColorScheme(scheme).frame(width: 700, height: height)
                let renderer = ImageRenderer(content: root)
                renderer.scale = 2
                let suffix = dark ? "dark" : "light"
                if let image = renderer.nsImage { try write(image, to: directory.appendingPathComponent("\(name)-\(suffix)-imagerenderer.png")) }
                let host = NSHostingView(rootView: root)
                host.frame = NSRect(x: 0, y: 0, width: 700, height: height)
                host.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
                let window = NSWindow(contentRect: host.frame, styleMask: [.borderless], backing: .buffered, defer: false)
                window.contentView = host
                window.setFrameOrigin(NSPoint(x: -10000, y: -10000))
                window.orderBack(nil)
                host.layoutSubtreeIfNeeded()
                try await Task.sleep(for: .milliseconds(350))
                host.layoutSubtreeIfNeeded()
                if let bitmap = host.bitmapImageRepForCachingDisplay(in: host.bounds) {
                    host.cacheDisplay(in: host.bounds, to: bitmap)
                    if let data = bitmap.representation(using: .png, properties: [:]) {
                        try data.write(to: directory.appendingPathComponent("\(name)-\(suffix).png"))
                    }
                }
                window.orderOut(nil)
                window.contentView = nil
                model.stop()
            }
        }
    }

    private static func write(_ image: NSImage, to url: URL) throws {
        guard let data = image.tiffRepresentation, let bitmap = NSBitmapImageRep(data: data),
              let png = bitmap.representation(using: .png, properties: [:]) else { return }
        try png.write(to: url)
    }
}
