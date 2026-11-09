import AppKit
import Foundation
import KudzuCore

/// Clipboard history plumbing. The model lives in KudzuCore's ``Clipboard``.
/// Port of `lib.rs` `pub mod clipboard` and `macos/clipboard.rs`.
public enum EngineClipboard {
    /// One pasteboard item: its types and their raw data, in pasteboard order.
    public typealias PasteItem = Clipboard.PasteItem

    /// A PNG thumbnail plus the original image's pixel size.
    public struct Thumbnail: Hashable, Sendable {
        public var png: Data
        public var width: Int
        public var height: Int

        public init(png: Data, width: Int, height: Int) {
            self.png = png
            self.width = width
            self.height = height
        }
    }

    /// Why pasting into another app didn't happen.
    public enum PasteError: Error, Hashable, Sendable {
        /// No Accessibility permission, or Secure Input is on.
        case notAllowed
        /// The app to paste into quit or couldn't be brought forward.
        case targetGone
    }

    /// The general pasteboard's change count; cheap enough to poll a few times a second.
    public static func changeCount() -> Int {
        Pasteboard.changeCount()
    }

    /// Whether a change count came from Kudzu's own writes (snippet pastes, restores, history picks).
    public static func isOwnChange(_ count: Int) -> Bool {
        Pasteboard.isOwnChange(count)
    }

    /// Every item and type on the pasteboard. May block briefly on data a source app provides lazily.
    public static func read() -> [PasteItem] {
        Pasteboard.readItems()
    }

    /// Replaces the pasteboard with `items`, recorded as Kudzu's own change. Returns the new change count.
    @discardableResult
    public static func write(_ items: [PasteItem]) -> Int {
        // A snippet paste still waiting to restore must not overwrite what the user picks now.
        Pasteboard.restorePending()
        return Pasteboard.writeItems(items)
    }

    /// Whether synthetic ⌘V can reach other apps right now.
    public static func canPaste() -> Bool {
        Keys.canPost()
    }

    /// Brings `target` forward (when given) and presses ⌘V. Blocks up to ~1 s: call off the UI thread.
    public static func paste(target: AppInfo?) throws(PasteError) {
        if let target {
            guard Apps.activateAndWait(target, timeout: 0.8) else { throw .targetGone }
            // Let the target's key window settle before ⌘V.
            Thread.sleep(forTimeInterval: 0.04)
        }
        do {
            try Keys.paste()
        } catch {
            throw .notAllowed
        }
    }

    /// Decodes image data (PNG, TIFF, JPEG, HEIC, PDF …) and renders a PNG at most `maxPx` on its
    /// longer side (never upscaled).
    public static func thumbnail(data: Data, maxPx: Int) -> Thumbnail? {
        autoreleasepool {
            guard let image = NSImage(data: data) else { return nil }
            // Pixel size from the largest bitmap representation; vector images use their point size.
            var (w, h) = (0.0, 0.0)
            for rep in image.representations {
                let (pw, ph) = (Double(rep.pixelsWide), Double(rep.pixelsHigh))
                if pw * ph > w * h {
                    (w, h) = (pw, ph)
                }
            }
            if w <= 0 || h <= 0 {
                (w, h) = (image.size.width, image.size.height)
            }
            guard w >= 1, h >= 1 else { return nil }
            let scale = min(Double(maxPx) / max(w, h), 1.0)
            let (tw, th) = (max(Int((w * scale).rounded()), 1), max(Int((h * scale).rounded()), 1))
            guard let png = Apps.drawPNG(image, width: tw, height: th) else { return nil }
            return Thumbnail(png: png, width: Int(w.rounded()), height: Int(h.rounded()))
        }
    }
}
