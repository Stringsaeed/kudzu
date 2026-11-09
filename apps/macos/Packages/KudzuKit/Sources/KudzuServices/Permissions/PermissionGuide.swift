import AppKit
import CoreGraphics
import KudzuCore

/// The permission companion's native pieces. Port of `permissions/{mod.rs,macos.m}`.
///
/// OS approval stays in System Settings; the companion guides the handoff: it opens the right
/// pane, moves itself beside the Settings window (window bounds metadata only: no screen
/// capture, no Accessibility needed to guide toward Accessibility), and plays a click-through
/// demo of dragging the app icon into the list. The demo stops on a real drag, a Settings
/// window move, switching apps, closing the companion, or Reduce Motion.
@MainActor
public final class PermissionGuide {
    /// The marker that reopens the companion after an explicit restart. It never stands in for
    /// an OS grant.
    public static let resumeFile = "permission-setup-pending"

    public weak var window: NSWindow?
    public private(set) var dragView: AppDragView?
    private var overlay: NSPanel?
    private var timer: Timer?
    private var generation = 0

    public init(window: NSWindow) {
        self.window = window
    }

    // MARK: Labels

    /// "Accessibility", or "Device Control and Data Access" on macOS 27 and later.
    public nonisolated static var accessibilityName: String {
        ProcessInfo.processInfo.operatingSystemVersion.majorVersion >= 27 ? "Device Control and Data Access" : "Accessibility"
    }

    // MARK: Drag icon

    /// Places the draggable icon over `bounds` (top-left origin, in the window's content
    /// coordinates, 1…128 pt). `false` when there's no app bundle to drag.
    public func installDragIcon(bounds: ScreenRect) throws(ServiceError) -> Bool {
        let values = [bounds.x, bounds.y, bounds.width, bounds.height]
        guard values.allSatisfy(\.isFinite), (1 ... 128).contains(bounds.width), (1 ... 128).contains(bounds.height) else {
            throw ServiceError("Invalid app icon bounds")
        }
        guard let window, AppDragView.applicationURL != nil, let content = window.contentView else { return false }
        let view: AppDragView
        if let existing = dragView {
            view = existing
        } else {
            view = AppDragView(frame: .zero)
            view.onInteraction = { [weak self] in self?.stop() }
            content.addSubview(view, positioned: .above, relativeTo: nil)
            dragView = view
        }
        let y = content.isFlipped ? bounds.y : content.bounds.height - bounds.y - bounds.height
        view.frame = NSRect(x: bounds.x, y: y, width: bounds.width, height: bounds.height)
        return true
    }

    // MARK: Settings

    /// Opens the Accessibility (or Input Monitoring) pane and, once its window appears,
    /// moves the companion beside it and plays the demo.
    public func openSettings(accessibility: Bool) -> Bool {
        stop()
        let pane = accessibility ? "Privacy_Accessibility" : "Privacy_ListenEvent"
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?\(pane)"),
              NSWorkspace.shared.open(url) else { return false }
        let gen = generation
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(450)) { [weak self] in
            self?.showBesideSettings(attempt: 0, generation: gen)
        }
        return true
    }

    /// Stops the demo and removes the overlay.
    public func stop() {
        generation += 1
        timer?.invalidate()
        timer = nil
        overlay?.orderOut(nil)
        overlay = nil
    }

    /// The visible System Settings window, in AppKit screen coordinates; `.zero` when none.
    public static func settingsFrame() -> NSRect {
        guard let settings = NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.systempreferences").first,
              let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]]
        else { return .zero }
        for info in list {
            guard (info[kCGWindowOwnerPID as String] as? Int32) == settings.processIdentifier,
                  (info[kCGWindowLayer as String] as? Int) == 0,
                  let dict = info[kCGWindowBounds as String] as? NSDictionary,
                  let bounds = CGRect(dictionaryRepresentation: dict),
                  bounds.width >= 400, bounds.height >= 300 else { continue }
            let desktopTop = NSScreen.screens.first?.frame.height ?? 0
            return NSRect(x: bounds.origin.x, y: desktopTop - bounds.maxY, width: bounds.width, height: bounds.height)
        }
        return .zero
    }

    private func showBesideSettings(attempt: Int, generation gen: Int) {
        guard gen == generation, let window, window.isVisible else { return }
        let destination = Self.settingsFrame()
        if destination.isEmpty {
            if attempt < 12 {
                DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(200)) { [weak self] in
                    self?.showBesideSettings(attempt: attempt + 1, generation: gen)
                }
            }
            return
        }
        let center = NSPoint(x: destination.midX, y: destination.midY)
        let screen = NSScreen.screens.first { $0.frame.contains(center) } ?? window.screen ?? NSScreen.main
        guard let available = screen?.visibleFrame else { return }
        let frame = window.frame
        var x = destination.minX - frame.width - 18
        if x < available.minX + 12 { x = destination.maxX + 18 }
        x = max(available.minX + 12, min(x, available.maxX - frame.width - 12))
        let y = max(available.minY + 12, min(destination.maxY - frame.height, available.maxY - frame.height - 12))
        window.setFrameOrigin(NSPoint(x: x, y: y))

        // On a small display a guide stays useful, but an overlapping ghost would be confusing.
        if window.frame.intersects(destination) || NSWorkspace.shared.accessibilityDisplayShouldReduceMotion { return }
        guard let dragView, let appURL = AppDragView.applicationURL else { return }
        let source = window.convertToScreen(dragView.convert(dragView.bounds, to: nil))
        let canvas = window.frame.union(destination).insetBy(dx: -80, dy: -80)

        let panel = NSPanel(contentRect: canvas, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.ignoresMouseEvents = true
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.isReleasedWhenClosed = false
        let drawing = GuideDrawing(frame: NSRect(origin: .zero, size: canvas.size))
        drawing.destination = destination.offsetBy(dx: -canvas.minX, dy: -canvas.minY)
        drawing.source = NSPoint(x: source.midX - canvas.minX, y: source.midY - canvas.minY)
        drawing.icon = NSWorkspace.shared.icon(forFile: appURL.path)
        panel.contentView = drawing
        overlay = panel
        panel.orderFrontRegardless()

        let start = Date.timeIntervalSinceReferenceDate
        let companionFrame = window.frame
        let ownID = Bundle.main.bundleIdentifier
        timer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30, repeats: true) { [weak self] t in
            let keep = MainActor.assumeIsolated { () -> Bool in
                guard let self, let window = self.window else { return false }
                let elapsed = Date.timeIntervalSinceReferenceDate - start
                let front = NSWorkspace.shared.frontmostApplication?.bundleIdentifier
                let relevant = front == "com.apple.systempreferences" || front == ownID
                if !window.isVisible || !relevant || elapsed > 4.8
                    || Self.settingsFrame() != destination || window.frame != companionFrame
                    || NSWorkspace.shared.accessibilityDisplayShouldReduceMotion {
                    self.stop()
                    return false
                }
                drawing.progress = CGFloat(fmod(elapsed, 2.4) / 2.4)
                drawing.needsDisplay = true
                return true
            }
            if !keep { t.invalidate() }
        }
    }

    // MARK: Finder, restart

    /// Shows the app bundle in Finder. `false` for a loose executable.
    public static func revealApp() -> Bool {
        guard let url = AppDragView.applicationURL else { return false }
        NSWorkspace.shared.activateFileViewerSelecting([url])
        return true
    }

    /// Writes the marker that reopens the companion after the next launch.
    public static func markResume(dataDir: URL) throws {
        try Data().write(to: dataDir.appendingPathComponent(resumeFile))
    }

    /// Removes the marker; true when it was there (show the companion again).
    public static func consumeResume(dataDir: URL) -> Bool {
        (try? FileManager.default.removeItem(at: dataDir.appendingPathComponent(resumeFile))) != nil
    }
}

/// The overlay's outline, dashed path and ghost icon (`KZGuideDrawing`).
private final class GuideDrawing: NSView {
    var destination = NSRect.zero
    var source = NSPoint.zero
    var progress: CGFloat = 0
    var icon: NSImage?

    override func draw(_ dirtyRect: NSRect) {
        let phase = progress
        let fade = min(1, min(phase * 10, (1 - phase) * 10))
        NSColor.controlAccentColor.withAlphaComponent(0.65 * fade).setStroke()
        let outline = NSBezierPath(roundedRect: destination.insetBy(dx: -3, dy: -3), xRadius: 14, yRadius: 14)
        outline.lineWidth = 3
        outline.stroke()
        // Point into the Settings window, without assuming the location of a specific OS control.
        let end = NSPoint(x: destination.midX + destination.width * 0.15, y: destination.midY)
        let bend = NSPoint(x: (source.x + end.x) / 2, y: max(source.y, end.y) + 65)
        let path = NSBezierPath()
        path.move(to: source)
        path.curve(to: end, controlPoint1: bend, controlPoint2: bend)
        let dash: [CGFloat] = [4, 6]
        path.setLineDash(dash, count: 2, phase: 0)
        path.lineWidth = 1.5
        path.stroke()
        var t = min(1, max(0, (phase - 0.12) / 0.65))
        t = t * t * (3 - 2 * t)
        let u = 1 - t
        let point = NSPoint(
            x: u * u * u * source.x + 3 * u * t * bend.x + t * t * t * end.x,
            y: u * u * u * source.y + 3 * u * t * bend.y + t * t * t * end.y
        )
        icon?.draw(in: NSRect(x: point.x - 26, y: point.y - 26, width: 52, height: 52), from: .zero, operation: .sourceOver, fraction: fade * 0.85)
    }
}
