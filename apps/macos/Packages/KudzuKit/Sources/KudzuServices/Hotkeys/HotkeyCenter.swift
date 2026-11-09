import Carbon.HIToolbox
import Foundation
import KudzuCore

/// Global shortcuts for quick search and clipboard history. Port of `hotkey.rs`.
///
/// Uses Carbon's `RegisterEventHotKey`, which (unlike an event tap) needs no Accessibility
/// permission and swallows the key press. Each ``Slot`` holds at most one shortcut; registering
/// replaces the slot's previous one, and a failure keeps the old one.
@MainActor
public final class HotkeyCenter {
    /// Which window a global shortcut opens.
    public enum Slot: UInt32, Sendable, CaseIterable {
        case palette = 1
        case clipboard = 2
    }

    /// The Carbon key code and modifier mask for an accelerator string.
    public struct Combo: Hashable, Sendable {
        public var keyCode: UInt32
        public var modifiers: UInt32
    }

    /// Called on the main thread when a slot's shortcut is pressed.
    public var onPress: ((Slot) -> Void)?

    private struct Registered {
        var accelerator: String
        var combo: Combo
        var ref: EventHotKeyRef
    }

    private var registered: [Slot: Registered] = [:]
    private var handler: EventHandlerRef?
    private static let signature: OSType = 0x4B5A_686B // 'KZhk'

    public init() {}

    // Carbon retains the callback's raw pointer, not this object. Remove it before releasing
    // self, and release every system registration owned by this center.
    isolated deinit {
        if let handler { RemoveEventHandler(handler) }
        for registration in registered.values { UnregisterEventHotKey(registration.ref) }
    }

    /// Normalizes the UI's accelerator ("Control+Alt+Space", "Command+Shift+Semicolon") and
    /// turns it into a Carbon key code and modifiers; `nil` when it isn't a valid shortcut.
    public nonisolated static func combo(for accelerator: String) -> Combo? {
        guard let a = Keys.parseAccelerator(accelerator) else { return nil }
        return Combo(keyCode: a.carbonKeyCode, modifiers: a.carbonModifiers)
    }

    /// The accelerator registered for `slot`, if any.
    public func accelerator(for slot: Slot) -> String? { registered[slot]?.accelerator }

    /// Registers `accelerator` for `slot`, replacing that slot's previous shortcut. On failure the
    /// old one stays.
    public func register(_ slot: Slot, accelerator: String) throws(ServiceError) {
        installHandler()
        guard let combo = Self.combo(for: accelerator) else {
            throw ServiceError("Invalid shortcut: couldn't recognize “\(accelerator)” as a shortcut.")
        }
        for (other, reg) in registered where other != slot && reg.combo == combo {
            throw ServiceError("Kudzu already uses that shortcut for something else.")
        }
        let previous = registered[slot]
        if previous?.accelerator == accelerator { return }
        if let previous {
            UnregisterEventHotKey(previous.ref)
            registered[slot] = nil
        }
        switch Self.carbonRegister(combo, slot: slot) {
        case let .success(ref):
            registered[slot] = Registered(accelerator: accelerator, combo: combo, ref: ref)
        case let .failure(status):
            if let previous, case let .success(ref) = Self.carbonRegister(previous.combo, slot: slot) {
                registered[slot] = Registered(accelerator: previous.accelerator, combo: previous.combo, ref: ref)
            }
            throw ServiceError("Couldn't use that shortcut — another app may own it (\(Self.describe(status))).")
        }
    }

    /// Removes `slot`'s shortcut (clipboard history turned off).
    public func unregister(_ slot: Slot) {
        if let previous = registered.removeValue(forKey: slot) {
            UnregisterEventHotKey(previous.ref)
        }
    }

    // MARK: Carbon

    private static func carbonRegister(_ combo: Combo, slot: Slot) -> Result<EventHotKeyRef, OSStatusError> {
        var ref: EventHotKeyRef?
        let id = EventHotKeyID(signature: signature, id: slot.rawValue)
        let status = RegisterEventHotKey(combo.keyCode, combo.modifiers, id, GetApplicationEventTarget(), 0, &ref)
        guard status == noErr, let ref else { return .failure(OSStatusError(status: status)) }
        return .success(ref)
    }

    struct OSStatusError: Error { var status: OSStatus }

    private static func describe(_ e: OSStatusError) -> String {
        e.status == OSStatus(eventHotKeyExistsErr) ? "the shortcut is already registered" : "error \(e.status)"
    }

    private func installHandler() {
        guard handler == nil else { return }
        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        let me = Unmanaged.passUnretained(self).toOpaque()
        InstallEventHandler(GetApplicationEventTarget(), { _, event, user -> OSStatus in
            guard let event, let user else { return OSStatus(eventNotHandledErr) }
            var id = EventHotKeyID()
            let status = GetEventParameter(
                event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID),
                nil, MemoryLayout<EventHotKeyID>.size, nil, &id
            )
            guard status == noErr, id.signature == HotkeyCenter.signature, let slot = Slot(rawValue: id.id) else {
                return OSStatus(eventNotHandledErr)
            }
            // Carbon delivers hot-key events on the main thread's event loop.
            MainActor.assumeIsolated {
                let center = Unmanaged<HotkeyCenter>.fromOpaque(user).takeUnretainedValue()
                center.onPress?(slot)
            }
            return noErr
        }, 1, &spec, me, &handler)
    }
}
