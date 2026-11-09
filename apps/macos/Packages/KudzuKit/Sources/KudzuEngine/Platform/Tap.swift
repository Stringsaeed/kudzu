import CoreGraphics
import Foundation
import KudzuCore

// A CGEventTap serviced by its own thread and CFRunLoop. Port of `macos/tap.rs`.

/// Decides whether to swallow an event. Runs on the tap thread; must be fast.
typealias TapHandler = @Sendable (CGEventType, CGEvent) -> Bool

/// State the C callback reaches through `userInfo`.
///
/// `@unchecked Sendable`: `handler` and `active` are immutable; `port` is written once on the
/// creating thread before the tap is enabled (so before any callback) and only read afterwards;
/// `stopped` is lock-guarded.
private final class TapContext: @unchecked Sendable {
    let handler: TapHandler
    let active: Bool
    var port: CFMachPort?
    let stopped = Locked(false)

    init(handler: @escaping TapHandler, active: Bool) {
        self.handler = handler
        self.active = active
    }
}

private func tapCallback(
    proxy: CGEventTapProxy,
    kind: CGEventType,
    event: CGEvent,
    userInfo: UnsafeMutableRawPointer?
) -> Unmanaged<CGEvent>? {
    guard let userInfo else { return Unmanaged.passUnretained(event) }
    // SAFETY: `userInfo` is the TapContext retained by the tap thread, alive while its run loop runs.
    let context = Unmanaged<TapContext>.fromOpaque(userInfo).takeUnretainedValue()
    if kind == .tapDisabledByTimeout || kind == .tapDisabledByUserInput {
        if let port = context.port, !context.stopped.get() {
            CGEvent.tapEnable(tap: port, enable: true)
        }
    }
    // Swift has no unwinding, so nothing can escape into CoreGraphics here.
    let swallow = context.handler(kind, event)
    return swallow && context.active ? nil : Unmanaged.passUnretained(event)
}

/// A running tap. Call ``stop()`` (or drop it) to stop the tap.
///
/// `@unchecked Sendable`: the port is only used for thread-safe enable/invalidate calls, and
/// the stop flag is lock-guarded.
final class TapHandle: @unchecked Sendable {
    private let port: CFMachPort
    private let stopped: Locked<Bool>
    /// How this tap was created.
    let mode: TapMode

    private static let eventMask: CGEventMask = {
        let kinds: [CGEventType] = [.keyDown, .keyUp, .leftMouseDown, .rightMouseDown, .otherMouseDown]
        return kinds.reduce(0) { $0 | (CGEventMask(1) << CGEventMask($1.rawValue)) }
    }()

    private init(port: CFMachPort, stopped: Locked<Bool>, mode: TapMode) {
        self.port = port
        self.stopped = stopped
        self.mode = mode
    }

    /// Creates a session tap (disabled until ``enable()``), serviced on a new thread.
    /// `nil` when macOS refuses (missing permission).
    static func start(_ mode: TapMode, handler: @escaping TapHandler) -> TapHandle? {
        let options: CGEventTapOptions
        switch mode {
        case .active: options = .defaultTap
        case .listenOnly: options = .listenOnly
        case .off: return nil
        }
        let context = TapContext(handler: handler, active: mode == .active)
        let opaque = Unmanaged.passUnretained(context).toOpaque()
        guard let port = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .tailAppendEventTap,
            options: options,
            eventsOfInterest: eventMask,
            callback: tapCallback,
            userInfo: opaque
        ) else { return nil }
        // A new tap starts enabled; an enabled active tap whose run loop isn't serviced yet
        // would stall every keystroke, so keep it off until the caller enables it.
        CGEvent.tapEnable(tap: port, enable: false)
        context.port = port
        guard let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, port, 0) else {
            CFMachPortInvalidate(port)
            return nil
        }
        let stopped = context.stopped
        let box = RunLoopBox(port: port, source: source, context: context)
        let thread = Thread { box.run() }
        thread.name = "kudzu-event-tap"
        thread.qualityOfService = .userInteractive
        thread.start()
        return TapHandle(port: port, stopped: stopped, mode: mode)
    }

    /// Starts delivering events.
    func enable() {
        if !stopped.get() {
            CGEvent.tapEnable(tap: port, enable: true)
        }
    }

    /// Stops the tap; its thread exits within a quarter second. Idempotent.
    func stop() {
        guard !stopped.swap(true) else { return }
        CGEvent.tapEnable(tap: port, enable: false)
        CFMachPortInvalidate(port)
    }

    deinit {
        stop()
    }
}

/// What the tap thread owns. `@unchecked Sendable`: handed to exactly one thread, which is the
/// only one touching the run loop source; the port is only enabled/invalidated (thread-safe).
private final class RunLoopBox: @unchecked Sendable {
    let port: CFMachPort
    let source: CFRunLoopSource
    let context: TapContext

    init(port: CFMachPort, source: CFRunLoopSource, context: TapContext) {
        self.port = port
        self.source = source
        self.context = context
    }

    /// Tap thread body: service the source until stopped, then release everything it owns.
    func run() {
        let runLoop = CFRunLoopGetCurrent()
        CFRunLoopAddSource(runLoop, source, .commonModes)
        while !context.stopped.get() {
            _ = autoreleasepool { CFRunLoopRunInMode(.defaultMode, 0.25, false) }
        }
        CFRunLoopRemoveSource(runLoop, source, .commonModes)
        CFMachPortInvalidate(port)
        // Callbacks only run inside this thread's run loop, which has stopped; `context` is
        // released with this box.
    }
}

/// A retained copy of a physical key event, held for later replay.
///
/// `@unchecked Sendable`: we own the copy and only touch it from one thread at a time (it moves
/// from the tap thread into the hold queue, then to whichever thread posts it).
final class HeldEvent: @unchecked Sendable {
    private let event: CGEvent
    /// Whether this is a key-down (as opposed to key-up).
    let isKeyDown: Bool

    private init(event: CGEvent, isKeyDown: Bool) {
        self.event = event
        self.isKeyDown = isKeyDown
    }

    /// Copies `event` (the original continues through the tap).
    static func copy(_ event: CGEvent, keyDown: Bool) -> HeldEvent? {
        event.copy().map { HeldEvent(event: $0, isKeyDown: keyDown) }
    }

    /// Posts the event with `marker` in its source user data.
    func post(_ marker: Int64) {
        event.setIntegerValueField(.eventSourceUserData, value: marker)
        event.post(tap: .cgSessionEventTap)
    }
}

enum TapEvents {
    /// The marker in an event's source user data.
    static func marker(_ event: CGEvent) -> Int64 {
        event.getIntegerValueField(.eventSourceUserData)
    }

    /// What a key-down means to the matcher; `nil` when it produces no text and changes no context
    /// (e.g. a dead key waiting for its base letter).
    static func classify(_ event: CGEvent) -> KeyInput? {
        let key = event.getIntegerValueField(.keyboardEventKeycode)
        var buffer = [UInt16](repeating: 0, count: 16)
        var length = 0
        buffer.withUnsafeMutableBufferPointer { pointer in
            event.keyboardGetUnicodeString(maxStringLength: pointer.count, actualStringLength: &length, unicodeString: pointer.baseAddress)
        }
        return classifyParts(flags: event.flags, key: key, text: Array(buffer.prefix(min(length, buffer.count))))
    }

    /// Pure part of ``classify(_:)``.
    static func classifyParts(flags: CGEventFlags, key: Int64, text: [UInt16]) -> KeyInput? {
        if !flags.intersection([.maskCommand, .maskControl]).isEmpty {
            return .reset
        }
        switch key {
        case 51:
            return .backspace
        // Escape, Home, Page Up, Forward Delete, End, Page Down, arrows.
        case 53, 115, 116, 117, 119, 121, 123...126:
            return .reset
        // Return / Enter and Tab are word separators rather than navigation.
        case 36, 76:
            return .character("\n")
        case 48:
            return .character("\t")
        default:
            break
        }
        if text.isEmpty {
            return nil
        }
        // Like Rust's `String::from_utf16`: unpaired surrogates are not text.
        var scalars = String.UnicodeScalarView()
        var decoder = UTF16()
        var iterator = text.makeIterator()
        decoding: while true {
            switch decoder.decode(&iterator) {
            case .scalarValue(let scalar): scalars.append(scalar)
            case .emptyInput: break decoding
            case .error: return .reset
            }
        }
        let control: (Unicode.Scalar) -> Bool = { $0.value < 32 || $0.value == 0x7F || (0xF700...0xF8FF).contains($0.value) }
        if scalars.contains(where: control) {
            return .reset
        }
        return .character(String(scalars))
    }
}
