import AppKit
import Observation
import KudzuCore

@MainActor @Observable
final class EventTap {
    enum Signal { case character(String), backspace, reset }
    nonisolated static let replayMarker: Int64 = 0x4B5552504C
    nonisolated static let syntheticMarker: Int64 = 0x4B55445A55
    var onMouseDown: ((CGEvent) -> Void)?
    var onSignal: ((Signal) -> Void)?
    var onKey: ((CGEvent, Signal?, Bool) -> Bool)?
    private(set) var mode: ExpansionTapMode?
    private(set) var status = "Waiting for Input Monitoring"
    private var port: CFMachPort?
    private var source: CFRunLoopSource?

    @discardableResult
    func start(mode: ExpansionTapMode = .listenOnly) -> Bool {
        if port != nil, self.mode == mode { return true }
        stop()
        let mask = [CGEventType.keyDown, .keyUp, .leftMouseDown, .rightMouseDown].reduce(CGEventMask(0)) {
            $0 | (CGEventMask(1) << $1.rawValue)
        }
        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap, place: .tailAppendEventTap, options: mode == .active ? .defaultTap : .listenOnly,
            eventsOfInterest: mask, callback: { _, type, event, context in
                // The source is installed exclusively on the main run loop.
                let swallow = MainActor.assumeIsolated {
                    if let context {
                        return Unmanaged<EventTap>.fromOpaque(context).takeUnretainedValue().receive(type, event)
                    }
                    return false
                }
                return swallow ? nil : Unmanaged.passUnretained(event)
            }, userInfo: Unmanaged.passUnretained(self).toOpaque()
        ) else {
            status = "Event tap unavailable (permission or sandbox)"
            return false
        }
        guard let runLoopSource = CFMachPortCreateRunLoopSource(nil, tap, 0) else {
            CFMachPortInvalidate(tap)
            status = "Event tap run loop failed"
            return false
        }
        self.mode = mode
        port = tap
        source = runLoopSource
        CFRunLoopAddSource(CFRunLoopGetMain(), runLoopSource, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        status = "Listening"
        return true
    }

    isolated deinit { stop() }

    func stop() {
        if let port { CFMachPortInvalidate(port) }
        if let source { CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .commonModes) }
        port = nil
        mode = nil
        source = nil
        status = "Stopped"
        onSignal?(.reset)
    }

    func reset() { onSignal?(.reset) }

    private func receive(_ type: CGEventType, _ event: CGEvent) -> Bool {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            onSignal?(.reset)
            if let port { CGEvent.tapEnable(tap: port, enable: true) }
            return false
        }
        let marker = event.getIntegerValueField(.eventSourceUserData)
        guard marker != Self.syntheticMarker else { return false }
        guard type == .keyDown || type == .keyUp else {
            if let onMouseDown { onMouseDown(event) } else { onSignal?(.reset) }
            return false
        }
        let swallow = onKey?(event, type == .keyDown ? signal(for: event) : nil,
                             marker == Self.replayMarker) ?? false
        return mode == .active && swallow
    }

    private func signal(for event: CGEvent) -> Signal {
        if !event.flags.intersection([.maskCommand, .maskControl]).isEmpty { return .reset }
        let key = event.getIntegerValueField(.keyboardEventKeycode)
        if key == 51 { return .backspace }
        if [53, 115, 116, 117, 119, 121, 123, 124, 125, 126].contains(key) { return .reset }
        // Return and tab are word separators, rather than navigation.
        if key == 36 || key == 76 { return .character("\n") }
        if key == 48 { return .character("\t") }
        guard let text = NSEvent(cgEvent: event)?.characters, !text.isEmpty,
              !text.unicodeScalars.contains(where: { $0.value < 32 || (0xF700...0xF8FF).contains($0.value) }) else {
            return .reset
        }
        return .character(text)
    }
}
