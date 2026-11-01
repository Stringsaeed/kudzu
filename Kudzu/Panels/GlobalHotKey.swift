import Carbon

/// Why a global shortcut couldn't be registered.
struct HotKeyRegistrationError: Error, Equatable {
    /// The Carbon status code.
    var status: OSStatus

    /// User-facing explanation.
    var message: String {
        switch Int(status) {
        case eventHotKeyExistsErr:
            "That shortcut is already used by another app. Choose a different one."
        case eventHotKeyInvalidErr:
            "That shortcut can't be used as a global shortcut."
        default:
            "Couldn't register the shortcut (error \(status))."
        }
    }
}

/// One system-wide hotkey, registered through Carbon (works sandboxed and needs no permissions).
@MainActor
final class GlobalHotKey {
    nonisolated private static let signature: OSType = 0x4B55445A // "KUDZ"
    private static var nextID: UInt32 = 1

    private var hotKey: EventHotKeyRef?
    private var handler: EventHandlerRef?
    private let identifier: UInt32
    private let action: () -> Void
    private var handlerStatus: OSStatus = noErr

    /// The shortcut currently registered, if any.
    private(set) var shortcut: HotKeyShortcut?

    /// Installs the event handler. Nothing fires until `register(_:)` succeeds.
    init(action: @escaping () -> Void) {
        self.action = action
        identifier = Self.nextID
        Self.nextID += 1
        var event = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        handlerStatus = InstallEventHandler(GetApplicationEventTarget(), { _, event, context in
            guard let event, let context else { return OSStatus(eventNotHandledErr) }
            var pressed = EventHotKeyID()
            let result = GetEventParameter(event, EventParamName(kEventParamDirectObject),
                                           EventParamType(typeEventHotKeyID), nil,
                                           MemoryLayout<EventHotKeyID>.size, nil, &pressed)
            guard result == noErr, pressed.signature == GlobalHotKey.signature else {
                return OSStatus(eventNotHandledErr)
            }
            return MainActor.assumeIsolated {
                let hotKey = Unmanaged<GlobalHotKey>.fromOpaque(context).takeUnretainedValue()
                // Every GlobalHotKey installs its own handler, so let the event reach the one it belongs to.
                guard pressed.id == hotKey.identifier else { return OSStatus(eventNotHandledErr) }
                hotKey.action()
                return noErr
            }
        }, 1, &event, Unmanaged.passUnretained(self).toOpaque(), &handler)
    }

    isolated deinit { stop() }

    /// Replaces the current registration with `shortcut`.
    ///
    /// On failure nothing is registered, so callers that want to keep the old shortcut must
    /// register it again.
    func register(_ shortcut: HotKeyShortcut) throws(HotKeyRegistrationError) {
        unregister()
        guard handlerStatus == noErr else { throw HotKeyRegistrationError(status: handlerStatus) }
        var ref: EventHotKeyRef?
        let status = RegisterEventHotKey(shortcut.keyCode, shortcut.modifiers,
                                         EventHotKeyID(signature: Self.signature, id: identifier),
                                         GetApplicationEventTarget(), 0, &ref)
        guard status == noErr, let ref else { throw HotKeyRegistrationError(status: status == noErr ? OSStatus(eventHotKeyInvalidErr) : status) }
        hotKey = ref
        self.shortcut = shortcut
    }

    /// Releases the shortcut but keeps the handler, so `register(_:)` can be called again.
    func unregister() {
        if let hotKey { UnregisterEventHotKey(hotKey) }
        hotKey = nil
        shortcut = nil
    }

    /// Releases everything.
    func stop() {
        unregister()
        if let handler { RemoveEventHandler(handler) }
        handler = nil
    }
}
