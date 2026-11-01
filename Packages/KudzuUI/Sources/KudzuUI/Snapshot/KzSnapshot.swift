import AppKit
import SwiftUI

/// Renders SwiftUI views offscreen to PNG, in Light and Dark, so changes can be reviewed without a screen.
///
/// Materials (`NSVisualEffectView`) can't render offscreen; `kzGlass` detects this through
/// `\.kzSnapshotRendering` and draws only its tint, so glass still reads correctly.
@MainActor
public enum KzSnapshot {
    /// The appearances a snapshot is rendered in.
    public enum Appearance: String, CaseIterable, Sendable {
        /// Light (Aqua).
        case light
        /// Dark (Dark Aqua).
        case dark

        /// The matching SwiftUI colour scheme.
        public var colorScheme: ColorScheme { self == .dark ? .dark : .light }

        /// The matching AppKit appearance.
        public var nsAppearance: NSAppearance? {
            NSAppearance(named: self == .dark ? .darkAqua : .aqua)
        }
    }

    /// Renders `view` at `scale` in `appearance` on a `background` token, proposing `width` (or the ideal width).
    public static func image<V: View>(
        of view: V,
        appearance: Appearance,
        width: CGFloat? = nil,
        scale: CGFloat = 2,
        background: KzColorToken = .surface
    ) -> CGImage? {
        KzFonts.register()
        let content = view
            .environment(\.colorScheme, appearance.colorScheme)
            .environment(\.kzSnapshotRendering, true)
            .background(background.color)
            .environment(\.colorScheme, appearance.colorScheme)
        let renderer = ImageRenderer(content: content)
        renderer.scale = scale
        renderer.isOpaque = false
        renderer.proposedSize = ProposedViewSize(width: width, height: nil)
        // Dynamic NSColors resolve against the current drawing appearance, so set it explicitly.
        var image: CGImage?
        (appearance.nsAppearance ?? NSAppearance.currentDrawing()).performAsCurrentDrawingAppearance {
            image = renderer.cgImage
        }
        return image
    }

    /// PNG data for a rendered image.
    public static func pngData(_ image: CGImage) -> Data? {
        let rep = NSBitmapImageRep(cgImage: image)
        return rep.representation(using: .png, properties: [:])
    }

    /// Renders `view` in Light and Dark and writes `<name>-light@2x.png` and `<name>-dark@2x.png` into `directory`.
    @discardableResult
    public static func write<V: View>(
        _ view: V,
        named name: String,
        to directory: URL,
        width: CGFloat? = nil,
        scale: CGFloat = 2,
        background: KzColorToken = .surface
    ) throws -> [URL] {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        var urls: [URL] = []
        for appearance in Appearance.allCases {
            guard let image = image(of: view, appearance: appearance, width: width, scale: scale, background: background),
                  let data = pngData(image) else {
                throw SnapshotError(message: "could not render \(name) (\(appearance.rawValue))")
            }
            let url = directory.appendingPathComponent("\(name)-\(appearance.rawValue)@\(Int(scale))x.png")
            try data.write(to: url)
            urls.append(url)
        }
        return urls
    }

    /// A rendering failure.
    public struct SnapshotError: Error, CustomStringConvertible {
        /// What failed.
        public let message: String
        /// Human-readable description.
        public var description: String { message }
    }
}
