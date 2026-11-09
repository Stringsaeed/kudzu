import ApplicationServices
import Carbon.HIToolbox
import CoreGraphics
import Foundation
import KudzuCore

// Permissions, Secure Input and Accessibility (caret, selection) queries. Port of `macos/ax.rs`.

enum AX {
    /// AX calls must never hang the caller on an unresponsive app.
    static let timeoutSeconds: Float = 0.2

    /// Accessibility and Input Monitoring grants.
    static func permissions() -> PermissionStatus {
        PermissionStatus(accessibility: AXIsProcessTrusted(), inputMonitoring: CGPreflightListenEventAccess())
    }

    /// Whether we may post synthetic events.
    static func postEventAllowed() -> Bool {
        CGPreflightPostEventAccess()
    }

    /// Shows the system Accessibility prompt (no-op when already trusted).
    static func requestAccessibility() {
        // The literal value of `kAXTrustedCheckOptionPrompt` (a mutable C global Swift 6 won't read).
        let options = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
        _ = AXIsProcessTrustedWithOptions(options)
    }

    /// Shows the system Input Monitoring prompt.
    static func requestInputMonitoring() {
        // Same TCC prompt as `IOHIDRequestAccess(kIOHIDRequestTypeListenEvent)`; calling both
        // would ask twice, so this keeps the Rust engine's CoreGraphics call.
        _ = CGRequestListenEventAccess()
    }

    /// Whether Secure Input is on, and the owning process id when the session reports it.
    static func secureInput() -> (on: Bool, pid: pid_t?) {
        guard IsSecureEventInputEnabled() else { return (false, nil) }
        // This undocumented session entry is advisory only; never gate detection on it.
        guard let dict = CGSessionCopyCurrentDictionary() as? [String: Any] else { return (true, nil) }
        let pid = (dict["kCGSSessionSecureInputPID"] as? NSNumber)?.int32Value
        return (true, pid.flatMap { $0 > 0 ? $0 : nil })
    }

    // MARK: - Elements

    private static func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
        var value: CFTypeRef?
        let error = AXUIElementCopyAttributeValue(element, name as CFString, &value)
        return error == .success ? value : nil
    }

    static func focusedElement() -> AXUIElement? {
        let system = AXUIElementCreateSystemWide()
        AXUIElementSetMessagingTimeout(system, timeoutSeconds)
        guard let focused = attribute(system, "AXFocusedUIElement"), CFGetTypeID(focused) == AXUIElementGetTypeID() else {
            return nil
        }
        // The type id check above makes this cast safe.
        let element = unsafeDowncast(focused, to: AXUIElement.self)
        AXUIElementSetMessagingTimeout(element, timeoutSeconds)
        return element
    }

    private static func axValue<T: BitwiseCopyable>(_ value: CFTypeRef?, _ kind: AXValueType, _ initial: T) -> T? {
        guard let value, CFGetTypeID(value) == AXValueGetTypeID() else { return nil }
        let axValue = unsafeDowncast(value, to: AXValue.self)
        var out = initial
        let ok = withUnsafeMutablePointer(to: &out) { AXValueGetValue(axValue, kind, UnsafeMutableRawPointer($0)) }
        return ok ? out : nil
    }

    private static func valid(_ rect: CGRect) -> Bool {
        [rect.origin.x, rect.origin.y, rect.size.width, rect.size.height].allSatisfy(\.isFinite)
            && rect.size.width >= 0 && rect.size.height > 0
    }

    private static func screenRect(_ rect: CGRect) -> ScreenRect {
        ScreenRect(x: rect.origin.x, y: rect.origin.y, width: rect.size.width, height: rect.size.height)
    }

    /// Caret bounds of the focused text element (global, top-left origin), else the element's frame.
    static func caretRect() -> ScreenRect? {
        guard let element = focusedElement() else { return nil }
        if let range = attribute(element, "AXSelectedTextRange") {
            var bounds: CFTypeRef?
            let error = AXUIElementCopyParameterizedAttributeValue(element, "AXBoundsForRange" as CFString, range, &bounds)
            if error == .success, let rect = axValue(bounds, .cgRect, CGRect.zero), valid(rect) {
                return screenRect(rect)
            }
        }
        guard let origin = axValue(attribute(element, "AXPosition"), .cgPoint, CGPoint.zero),
              let size = axValue(attribute(element, "AXSize"), .cgSize, CGSize.zero)
        else { return nil }
        let rect = CGRect(origin: origin, size: size)
        return valid(rect) ? screenRect(rect) : nil
    }

    /// Selected text of the focused element, when the app exposes it.
    static func selectedText() -> String? {
        guard let element = focusedElement(), let value = attribute(element, "AXSelectedText"),
              CFGetTypeID(value) == CFStringGetTypeID()
        else { return nil }
        let text = unsafeDowncast(value, to: CFString.self) as String
        return text.isEmpty ? nil : text
    }

    // MARK: - Caret snapshot

    private static func collapsedOffset(_ element: AXUIElement) -> Int? {
        guard let range = axValue(attribute(element, "AXSelectedTextRange"), .cfRange, CFRange(location: 0, length: 0)),
              range.length == 0, range.location >= 0
        else { return nil }
        return range.location
    }

    /// Only offsets are read; the user's surrounding text is never captured.
    static func caretSnapshot() -> CaretSnapshot? {
        guard let element = focusedElement(), let location = collapsedOffset(element) else { return nil }
        return CaretSnapshot(element: element, location: location)
    }

    /// Where the caret was before an insertion, to measure what the app actually inserted.
    struct CaretSnapshot {
        let element: AXUIElement
        let location: Int

        /// UTF-16 units inserted since the snapshot, when the same element still has focus.
        func insertedUTF16() -> Int? {
            guard let focused = AX.focusedElement(), CFEqual(focused, element), let now = AX.collapsedOffset(focused),
                  now >= location
            else { return nil }
            return now - location
        }
    }
}
