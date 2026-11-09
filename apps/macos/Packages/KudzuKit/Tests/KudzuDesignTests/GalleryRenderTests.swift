// Opt-in review artifact export, never a pixel snapshot assertion.
// KZ_RENDER_GALLERY=1 swift test --filter KudzuDesignTests/GalleryRenderTests
import AppKit
import CoreText
import SwiftUI
import Testing
@testable import KudzuDesign

@Suite(.serialized) struct GalleryRenderTests {
    @Test @MainActor func renderGalleryForReview() async throws {
        guard ProcessInfo.processInfo.environment["KZ_RENDER_GALLERY"] == "1" else { return }
        _ = NSApplication.shared
        let output = URL(fileURLWithPath: "/tmp/kz-v3a")
        try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
        let package = URL(fileURLWithPath: #filePath).resolvingSymlinksInPath().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let font = package.appendingPathComponent("../../Kudzu/Fonts/PaperMono-Variable.ttf").standardizedFileURL
        if FileManager.default.fileExists(atPath: font.path) { CTFontManagerRegisterFontsForURL(font as CFURL, .process, nil) }
        for dark in [false, true] {
            for section in DesignGallery.Section.allCases {
                let size = NSSize(width: 1000, height: 1500)
                let view = NSHostingView(rootView: DesignGallery(section: section)
                    .environment(\.colorScheme, dark ? .dark : .light)
                    .transaction { $0.animation = nil; $0.disablesAnimations = true }
                    .frame(width: size.width, height: size.height))
                view.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
                let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.borderless], backing: .buffered, defer: false)
                window.contentView = view
                view.frame = NSRect(origin: .zero, size: size)
                view.layoutSubtreeIfNeeded()
                try await Task.sleep(for: .milliseconds(650))
                view.layoutSubtreeIfNeeded()
                let rep = try #require(view.bitmapImageRepForCachingDisplay(in: view.bounds))
                view.cacheDisplay(in: view.bounds, to: rep)
                let data = try #require(rep.representation(using: .png, properties: [:]))
                let file = output.appendingPathComponent("gallery-\(section.rawValue.lowercased())-\(dark ? "dark" : "light").png")
                try data.write(to: file)
                print("Rendered \(file.path)")
                window.contentView = nil
            }
        }
    }
}
