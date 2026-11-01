import AppKit
import Carbon
import SwiftUI

/// A global keyboard shortcut: a virtual key code plus Carbon modifier flags.
struct HotKeyShortcut: Codable, Hashable, Sendable {
    /// Virtual key code (`kVK_*`), independent of the keyboard layout.
    var keyCode: UInt32
    /// Carbon modifier mask (`cmdKey | optionKey | controlKey | shiftKey`).
    var modifiers: UInt32

    /// ⌃⌥Space, the search palette's default.
    static let paletteDefault = HotKeyShortcut(keyCode: UInt32(kVK_Space), modifiers: UInt32(controlKey | optionKey))

    init(keyCode: UInt32, modifiers: UInt32) {
        self.keyCode = keyCode
        self.modifiers = modifiers
    }

    /// The shortcut a key-down event describes, or nil when it isn't usable globally.
    ///
    /// A shortcut needs ⌘, ⌃ or ⌥ (⇧ alone would swallow capital letters system-wide); function
    /// keys F1–F20 may stand alone.
    init?(event: NSEvent) {
        guard event.type == .keyDown else { return nil }
        let flags = event.modifierFlags.intersection([.command, .option, .control, .shift])
        let code = UInt32(event.keyCode)
        guard !Self.modifierKeyCodes.contains(Int(code)) else { return nil }
        guard !flags.intersection([.command, .option, .control]).isEmpty || Self.functionKeyNames[Int(code)] != nil else { return nil }
        self.init(keyCode: code, modifiers: Self.carbonModifiers(flags))
    }

    /// The modifiers as AppKit flags.
    var modifierFlags: NSEvent.ModifierFlags {
        var flags: NSEvent.ModifierFlags = []
        if modifiers & UInt32(controlKey) != 0 { flags.insert(.control) }
        if modifiers & UInt32(optionKey) != 0 { flags.insert(.option) }
        if modifiers & UInt32(shiftKey) != 0 { flags.insert(.shift) }
        if modifiers & UInt32(cmdKey) != 0 { flags.insert(.command) }
        return flags
    }

    /// Display form in the standard menu order, e.g. "⌃⌥Space".
    var displayString: String {
        Self.modifierSymbols(modifierFlags) + keyName
    }

    /// The key's printed name: "Space", "F5", "↩", or the character it types in the current layout.
    var keyName: String {
        let code = Int(keyCode)
        if let name = Self.specialKeyNames[code] ?? Self.functionKeyNames[code] { return name }
        return Self.layoutCharacter(for: UInt16(keyCode))?.uppercased() ?? "Key \(keyCode)"
    }

    /// The SwiftUI key equivalent, for showing the shortcut in a menu; nil when SwiftUI can't express the key.
    var keyEquivalent: KeyEquivalent? {
        switch Int(keyCode) {
        case kVK_Space: return .space
        case kVK_Return: return .return
        case kVK_Tab: return .tab
        case kVK_Delete: return .delete
        case kVK_ForwardDelete: return .deleteForward
        case kVK_Escape: return .escape
        case kVK_UpArrow: return .upArrow
        case kVK_DownArrow: return .downArrow
        case kVK_LeftArrow: return .leftArrow
        case kVK_RightArrow: return .rightArrow
        case kVK_Home: return .home
        case kVK_End: return .end
        case kVK_PageUp: return .pageUp
        case kVK_PageDown: return .pageDown
        default:
            guard Self.functionKeyNames[Int(keyCode)] == nil,
                  let character = Self.layoutCharacter(for: UInt16(keyCode))?.lowercased().first else { return nil }
            return KeyEquivalent(character)
        }
    }

    /// The modifiers as SwiftUI event modifiers.
    var eventModifiers: SwiftUI.EventModifiers {
        var result: SwiftUI.EventModifiers = []
        let flags = modifierFlags
        if flags.contains(.control) { result.insert(.control) }
        if flags.contains(.option) { result.insert(.option) }
        if flags.contains(.shift) { result.insert(.shift) }
        if flags.contains(.command) { result.insert(.command) }
        return result
    }

    // MARK: - Persistence

    private static let paletteDefaultsKey = "searchPaletteShortcut"

    /// The saved search palette shortcut, or the default.
    static func savedPaletteShortcut(in defaults: UserDefaults = .standard) -> HotKeyShortcut {
        guard let data = defaults.data(forKey: paletteDefaultsKey),
              let shortcut = try? JSONDecoder().decode(HotKeyShortcut.self, from: data) else { return paletteDefault }
        return shortcut
    }

    /// Saves the search palette shortcut; saving the default removes the key.
    static func savePaletteShortcut(_ shortcut: HotKeyShortcut, in defaults: UserDefaults = .standard) {
        if shortcut == paletteDefault {
            defaults.removeObject(forKey: paletteDefaultsKey)
        } else if let data = try? JSONEncoder().encode(shortcut) {
            defaults.set(data, forKey: paletteDefaultsKey)
        }
    }

    // MARK: - Helpers

    /// "⌃⌥⇧⌘" symbols for the given flags, in the standard order.
    static func modifierSymbols(_ flags: NSEvent.ModifierFlags) -> String {
        var symbols = ""
        if flags.contains(.control) { symbols += "⌃" }
        if flags.contains(.option) { symbols += "⌥" }
        if flags.contains(.shift) { symbols += "⇧" }
        if flags.contains(.command) { symbols += "⌘" }
        return symbols
    }

    private static func carbonModifiers(_ flags: NSEvent.ModifierFlags) -> UInt32 {
        var mask = 0
        if flags.contains(.control) { mask |= controlKey }
        if flags.contains(.option) { mask |= optionKey }
        if flags.contains(.shift) { mask |= shiftKey }
        if flags.contains(.command) { mask |= cmdKey }
        return UInt32(mask)
    }

    private static let modifierKeyCodes: Set<Int> = [
        kVK_Command, kVK_RightCommand, kVK_Shift, kVK_RightShift, kVK_Option, kVK_RightOption,
        kVK_Control, kVK_RightControl, kVK_CapsLock, kVK_Function,
    ]

    private static let specialKeyNames: [Int: String] = [
        kVK_Space: "Space", kVK_Return: "↩", kVK_ANSI_KeypadEnter: "⌤", kVK_Tab: "⇥", kVK_Delete: "⌫",
        kVK_ForwardDelete: "⌦", kVK_Escape: "⎋", kVK_UpArrow: "↑", kVK_DownArrow: "↓",
        kVK_LeftArrow: "←", kVK_RightArrow: "→", kVK_Home: "↖", kVK_End: "↘",
        kVK_PageUp: "⇞", kVK_PageDown: "⇟", kVK_Help: "Help",
    ]

    private static let functionKeyNames: [Int: String] = [
        kVK_F1: "F1", kVK_F2: "F2", kVK_F3: "F3", kVK_F4: "F4", kVK_F5: "F5", kVK_F6: "F6",
        kVK_F7: "F7", kVK_F8: "F8", kVK_F9: "F9", kVK_F10: "F10", kVK_F11: "F11", kVK_F12: "F12",
        kVK_F13: "F13", kVK_F14: "F14", kVK_F15: "F15", kVK_F16: "F16", kVK_F17: "F17",
        kVK_F18: "F18", kVK_F19: "F19", kVK_F20: "F20",
    ]

    /// The character a key types with no modifiers in the current keyboard layout.
    private static func layoutCharacter(for keyCode: UInt16) -> String? {
        guard let source = TISCopyCurrentKeyboardLayoutInputSource()?.takeRetainedValue(),
              let pointer = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData) else { return nil }
        let data = Unmanaged<CFData>.fromOpaque(pointer).takeUnretainedValue() as Data
        return data.withUnsafeBytes { raw -> String? in
            guard let layout = raw.baseAddress?.assumingMemoryBound(to: UCKeyboardLayout.self) else { return nil }
            var deadKeys: UInt32 = 0
            var length = 0
            var characters = [UniChar](repeating: 0, count: 4)
            let status = UCKeyTranslate(layout, keyCode, UInt16(kUCKeyActionDisplay), 0, UInt32(LMGetKbdType()),
                                        OptionBits(kUCKeyTranslateNoDeadKeysBit), &deadKeys,
                                        characters.count, &length, &characters)
            guard status == noErr, length > 0 else { return nil }
            let string = String(utf16CodeUnits: characters, count: length)
            return string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : string
        }
    }
}
