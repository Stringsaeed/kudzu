import AppKit

/// The draggable app icon of the permission companion (`KZAppDragView` in
/// `permissions/macos.m`). A real `NSView` owns the gesture: dragging it into System Settings'
/// Accessibility list adds Kudzu, because its drag pasteboard holds `Bundle.main.bundleURL`, so
/// it always represents the running application (including Kudzu Dev). A click reveals the app
/// in Finder instead.
///
/// Transparent: the companion draws the icon itself and places this view over it (or embeds it
/// with an `NSViewRepresentable`).
@MainActor
public final class AppDragView: NSView, NSDraggingSource {
    /// Called when a drag or click starts, so the demo animation stops.
    public var onInteraction: (() -> Void)?
    private var downEvent: NSEvent?

    public override init(frame: NSRect) {
        super.init(frame: frame)
        // The companion's button supplies the keyboard alternative.
        setAccessibilityElement(false)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    /// The `.app` folder this process runs from; `nil` for a loose executable (nothing to drag).
    public static var applicationURL: URL? {
        let url = Bundle.main.bundleURL
        return url.pathExtension.lowercased() == "app" ? url : nil
    }

    public override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    public override func resetCursorRects() {
        addCursorRect(bounds, cursor: .openHand)
    }

    public override func mouseDown(with event: NSEvent) {
        downEvent = event
    }

    public override func mouseUp(with event: NSEvent) {
        if downEvent != nil, let url = Self.applicationURL {
            onInteraction?()
            NSWorkspace.shared.activateFileViewerSelecting([url])
        }
        downEvent = nil
    }

    public override func mouseDragged(with event: NSEvent) {
        guard let down = downEvent, let url = Self.applicationURL else { return }
        let dx = event.locationInWindow.x - down.locationInWindow.x
        let dy = event.locationInWindow.y - down.locationInWindow.y
        guard hypot(dx, dy) >= 4 else { return }
        onInteraction?()
        let item = NSDraggingItem(pasteboardWriter: url as NSURL)
        item.setDraggingFrame(bounds, contents: NSWorkspace.shared.icon(forFile: url.path))
        let session = beginDraggingSession(with: [item], event: down, source: self)
        session.animatesToStartingPositionsOnCancelOrFail = true
        downEvent = nil
    }

    public func draggingSession(_ session: NSDraggingSession, sourceOperationMaskFor context: NSDraggingContext) -> NSDragOperation {
        .copy
    }
}
