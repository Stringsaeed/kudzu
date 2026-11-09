import Foundation
import KudzuCore
import KudzuEngine

/// The pasteboard and app lookups clipboard history needs. ``LiveClipboardSystem`` goes through
/// KudzuEngine; tests substitute a fake.
public protocol ClipboardSystem: Sendable {
    func changeCount() -> Int
    func isOwnChange(_ count: Int) -> Bool
    func read() -> [Clipboard.PasteItem]
    func write(_ items: [Clipboard.PasteItem])
    func canPaste() -> Bool
    /// Brings `target` forward and presses ⌘V. Blocks up to ~1 s.
    func paste(target: AppInfo?) throws(EngineClipboard.PasteError)
    func thumbnail(_ data: Data, maxPx: Int) -> EngineClipboard.Thumbnail?
    func frontmostApp() -> AppInfo?
    func iconPNG(path: String, size: Int) -> Data?
}

/// The real pasteboard, through KudzuEngine.
public struct LiveClipboardSystem: ClipboardSystem {
    public init() {}
    public func changeCount() -> Int { EngineClipboard.changeCount() }
    public func isOwnChange(_ count: Int) -> Bool { EngineClipboard.isOwnChange(count) }
    public func read() -> [Clipboard.PasteItem] { EngineClipboard.read() }
    public func write(_ items: [Clipboard.PasteItem]) { EngineClipboard.write(items) }
    public func canPaste() -> Bool { EngineClipboard.canPaste() }
    public func paste(target: AppInfo?) throws(EngineClipboard.PasteError) { try EngineClipboard.paste(target: target) }
    public func thumbnail(_ data: Data, maxPx: Int) -> EngineClipboard.Thumbnail? { EngineClipboard.thumbnail(data: data, maxPx: maxPx) }
    public func frontmostApp() -> AppInfo? { System.frontmostApp() }
    public func iconPNG(path: String, size: Int) -> Data? { System.appIconPNG(path: path, size: size) }
}
