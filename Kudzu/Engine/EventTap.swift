import AppKit
import Observation

@MainActor @Observable
final class EventTap {
    enum Signal { case character(String), backspace, reset }
    nonisolated static let syntheticMarker: Int64 = 0x4B55445A55
    var onSignal: ((Signal) -> Void)?
    private(set) var status = "Waiting for Input Monitoring"
    private var port: CFMachPort?
    private var source: CFRunLoopSource?

    func start() {
        guard port == nil else { return }
        let mask = [CGEventType.keyDown, .leftMouseDown, .rightMouseDown].reduce(CGEventMask(0)) {
            $0 | (CGEventMask(1) << $1.rawValue)
        }
        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap, place: .tailAppendEventTap, options: .listenOnly,
            eventsOfInterest: mask, callback: { _, type, event, context in
                // The source is installed exclusively on the main run loop.
                MainActor.assumeIsolated {
                    if let context {
                        Unmanaged<EventTap>.fromOpaque(context).takeUnretainedValue().receive(type, event)
                    }
                }
                return Unmanaged.passUnretained(event)
            }, userInfo: Unmanaged.passUnretained(self).toOpaque()
        ) else {
            status = "Event tap unavailable (permission or sandbox)"
            return
        }
        guard let runLoopSource = CFMachPortCreateRunLoopSource(nil, tap, 0) else {
            CFMachPortInvalidate(tap)
            status = "Event tap run loop failed"
            return
        }
        port = tap
        source = runLoopSource
        CFRunLoopAddSource(CFRunLoopGetMain(), runLoopSource, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        status = "Listening"
    }

    isolated deinit { stop() }

    func stop() {
        if let port { CFMachPortInvalidate(port) }
        if let source { CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .commonModes) }
        port = nil
        source = nil
        status = "Stopped"
        onSignal?(.reset)
    }

    func reset() { onSignal?(.reset) }

    private func receive(_ type: CGEventType, _ event: CGEvent) {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            onSignal?(.reset)
            if let port { CGEvent.tapEnable(tap: port, enable: true) }
            return
        }
        guard event.getIntegerValueField(.eventSourceUserData) != Self.syntheticMarker else { return }
        guard type == .keyDown else { onSignal?(.reset); return }
        if !event.flags.intersection([.maskCommand, .maskControl]).isEmpty {
            onSignal?(.reset)
            return
        }
        let key = event.getIntegerValueField(.keyboardEventKeycode)
        if key == 51 { onSignal?(.backspace); return }
        // Navigation, escape, forward delete, return and tab invalidate the suffix.
        if [36, 48, 53, 76, 115, 116, 117, 119, 121, 123, 124, 125, 126].contains(key) {
            onSignal?(.reset)
            return
        }
        // AppKit translates using the active keyboard layout; input-method composition is separate.
        guard let text = NSEvent(cgEvent: event)?.characters, !text.isEmpty,
              !text.unicodeScalars.contains(where: { $0.value < 32 || (0xF700...0xF8FF).contains($0.value) }) else {
            onSignal?(.reset)
            return
        }
        onSignal?(.character(text))
    }
}
