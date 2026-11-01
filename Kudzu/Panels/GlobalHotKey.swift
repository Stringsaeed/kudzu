import Carbon

@MainActor
final class GlobalHotKey {
    private var hotKey: EventHotKeyRef?
    private var handler: EventHandlerRef?
    private let action: () -> Void
    private(set) var status: OSStatus = noErr

    init(action: @escaping () -> Void) {
        self.action = action
        var event = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        status = InstallEventHandler(GetApplicationEventTarget(), { _, event, context in
            guard let event, let context else { return OSStatus(eventNotHandledErr) }
            var identifier = EventHotKeyID()
            let result = GetEventParameter(event, EventParamName(kEventParamDirectObject),
                                           EventParamType(typeEventHotKeyID), nil,
                                           MemoryLayout<EventHotKeyID>.size, nil, &identifier)
            guard result == noErr, identifier.signature == 0x4B55445A, identifier.id == 1 else {
                return OSStatus(eventNotHandledErr)
            }
            MainActor.assumeIsolated {
                Unmanaged<GlobalHotKey>.fromOpaque(context).takeUnretainedValue().action()
            }
            return noErr
        }, 1, &event, Unmanaged.passUnretained(self).toOpaque(), &handler)
        if status == noErr {
            status = RegisterEventHotKey(UInt32(kVK_Space), UInt32(controlKey | optionKey),
                                        EventHotKeyID(signature: 0x4B55445A, id: 1),
                                        GetApplicationEventTarget(), 0, &hotKey)
        }
    }

    isolated deinit { stop() }

    func stop() {
        if let hotKey { UnregisterEventHotKey(hotKey) }
        if let handler { RemoveEventHandler(handler) }
        hotKey = nil
        handler = nil
    }
}
