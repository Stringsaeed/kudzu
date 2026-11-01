import AppKit
import ApplicationServices

@MainActor
struct CaretLocator {
    enum Source: String { case selection = "AX selection", element = "AX element", mouse = "Mouse fallback" }
    struct Location { let rect: NSRect; let source: Source }

    func locate() -> Location {
        let system = AXUIElementCreateSystemWide()
        AXUIElementSetMessagingTimeout(system, 0.2)
        var focused: CFTypeRef?
        if AXUIElementCopyAttributeValue(system, kAXFocusedUIElementAttribute as CFString, &focused) == .success,
           let focused, CFGetTypeID(focused) == AXUIElementGetTypeID() {
            let element = focused as! AXUIElement
            AXUIElementSetMessagingTimeout(element, 0.2)
            var range: CFTypeRef?
            var bounds: CFTypeRef?
            if AXUIElementCopyAttributeValue(element, kAXSelectedTextRangeAttribute as CFString, &range) == .success,
               let range,
               AXUIElementCopyParameterizedAttributeValue(element, kAXBoundsForRangeParameterizedAttribute as CFString,
                                                          range, &bounds) == .success,
               let rect = rectangle(bounds) {
                return Location(rect: appKitRect(rect), source: .selection)
            }
            var position: CFTypeRef?
            var size: CFTypeRef?
            if AXUIElementCopyAttributeValue(element, kAXPositionAttribute as CFString, &position) == .success,
               AXUIElementCopyAttributeValue(element, kAXSizeAttribute as CFString, &size) == .success,
               let position, let size,
               CFGetTypeID(position) == AXValueGetTypeID(), CFGetTypeID(size) == AXValueGetTypeID() {
                var point = CGPoint.zero
                var dimensions = CGSize.zero
                if AXValueGetValue(position as! AXValue, .cgPoint, &point),
                   AXValueGetValue(size as! AXValue, .cgSize, &dimensions) {
                    let rect = CGRect(origin: point, size: dimensions)
                    if valid(rect) { return Location(rect: appKitRect(rect), source: .element) }
                }
            }
        }
        return Location(rect: NSRect(origin: NSEvent.mouseLocation, size: .zero), source: .mouse)
    }

    private func rectangle(_ value: CFTypeRef?) -> CGRect? {
        guard let value, CFGetTypeID(value) == AXValueGetTypeID() else { return nil }
        var rect = CGRect.zero
        guard AXValueGetValue(value as! AXValue, .cgRect, &rect), valid(rect) else { return nil }
        return rect
    }

    private func valid(_ rect: CGRect) -> Bool {
        rect.origin.x.isFinite && rect.origin.y.isFinite && rect.width.isFinite && rect.height.isFinite
            && rect.width >= 0 && rect.height > 0
    }

    private func appKitRect(_ rect: CGRect) -> NSRect {
        // AX uses the primary display's top-left, even for displays above or left of it.
        let primaryTop = NSScreen.screens.first?.frame.maxY ?? 0
        return NSRect(x: rect.minX, y: primaryTop - rect.maxY, width: rect.width, height: rect.height)
    }
}
