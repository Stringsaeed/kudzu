import Foundation

/// Platform-neutral representation of the macOS event modifier bits (same raw values as NSEvent).
public struct KeyModifiers: OptionSet, Hashable, Sendable {
    public var rawValue: UInt64
    public init(rawValue: UInt64) { self.rawValue = rawValue }
    public static let capsLock = Self(rawValue: 1 << 16)
    public static let shift = Self(rawValue: 1 << 17)
    public static let control = Self(rawValue: 1 << 18)
    public static let option = Self(rawValue: 1 << 19)
    public static let command = Self(rawValue: 1 << 20)
    public static let function = Self(rawValue: 1 << 23)
}
public struct KeyEvent: Hashable, Sendable {
    public var keyCode: UInt16
    public var characters: String
    public var modifiers: KeyModifiers
    public init(keyCode: UInt16, characters: String = "", modifiers: KeyModifiers = []) { self.keyCode = keyCode; self.characters = characters; self.modifiers = modifiers }
    public func matches(combo: String) -> Bool { Keys.matches(self, combo: combo) }
}
public struct Accelerator: Hashable, Sendable {
    public var keyCode: UInt16
    public var modifiers: KeyModifiers
    /// Carbon RegisterEventHotKey modifier mask: cmdKey, shiftKey, optionKey, controlKey.
    public var carbonModifiers: UInt32 {
        (modifiers.contains(.command) ? 256 : 0) | (modifiers.contains(.shift) ? 512 : 0) |
        (modifiers.contains(.option) ? 2048 : 0) | (modifiers.contains(.control) ? 4096 : 0)
    }
    public var carbonKeyCode: UInt32 { UInt32(keyCode) }
}
/// One shortcut grammar shared by event handlers and hint labels.
public enum Keys {
    private static let symbols = ["mod": "⌘", "cmd": "⌘", "ctrl": "⌃", "control": "⌃", "alt": "⌥", "option": "⌥", "shift": "⇧", "enter": "↵", "backspace": "⌫", "delete": "⌫", "escape": "esc", "up": "↑", "down": "↓", "left": "←", "right": "→", "tab": "⇥", "space": "Space"]
    /// "mod+shift+k" → ["⌘", "⇧", "K"]. Like `k.length === 1`, only a single UTF-16 unit is uppercased.
    public static func keyLabels(_ combo: String) -> [String] { combo.components(separatedBy: "+").map { symbols[$0.lowercased()] ?? ($0.utf16.count == 1 ? $0.uppercased() : $0) } }
    public static func acceleratorLabels(_ accel: String) -> [String] {
        accel.components(separatedBy: "+").map { p in
            if ["commandorcontrol", "cmdorctrl", "super", "command", "cmd", "meta"].contains(p.lowercased()) { return "⌘" }
            return symbols[p.lowercased()] ?? (p.utf16.count == 1 ? p.uppercased() : p)
        }
    }
    /// Builds a Tauri accelerator or nil for unsupported keys, modifier-only input, bare keys or Shift-only keys.
    public static func acceleratorFromEvent(_ event: KeyEvent) -> String? {
        guard let key = codes[event.keyCode], !["Escape", "Backspace", "Delete", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].contains(key) else { return nil }
        var mods: [String] = []
        if event.modifiers.contains(.control) { mods.append("Control") }
        if event.modifiers.contains(.option) { mods.append("Alt") }
        if event.modifiers.contains(.shift) { mods.append("Shift") }
        if event.modifiers.contains(.command) { mods.append("Command") }
        let f = key.hasPrefix("F") && Int(key.dropFirst()) != nil
        if !f && (mods.isEmpty || mods == ["Shift"]) { return nil }
        let name = key.hasPrefix("Key") ? String(key.dropFirst(3)) : key.hasPrefix("Digit") ? String(key.dropFirst(5)) : key
        return (mods + [name]).joined(separator: "+")
    }
    public static func acceleratorFromEvent(keyCode: UInt16, modifiers: KeyModifiers) -> String? { acceleratorFromEvent(KeyEvent(keyCode: keyCode, modifiers: modifiers)) }
    public static func parseAccelerator(_ accel: String) -> Accelerator? {
        let parts = accel.components(separatedBy: "+"); guard let last = parts.last, !last.isEmpty else { return nil }
        var mods: KeyModifiers = []
        for part in parts.dropLast() {
            switch part.lowercased() {
            case "mod", "commandorcontrol", "cmdorctrl", "super", "command", "cmd", "meta": mods.insert(.command)
            case "ctrl", "control": mods.insert(.control)
            case "alt", "option": mods.insert(.option)
            case "shift": mods.insert(.shift)
            default: return nil
            }
        }
        var name = last.lowercased()
        let aliases = ["return":"enter", "esc":"escape", "up":"arrowup", "down":"arrowdown", "left":"arrowleft", "right":"arrowright", "del":"delete"]
        name = aliases[name] ?? name
        guard let code = codes.first(where: { key, value in
            let v = value.lowercased()
            return v == name || (v.hasPrefix("key") && String(v.dropFirst(3)) == name) || (v.hasPrefix("digit") && String(v.dropFirst(5)) == name)
        })?.key else { return nil }
        return Accelerator(keyCode: code, modifiers: mods)
    }
    public static func matches(_ event: KeyEvent, combo: String) -> Bool {
        let parts = combo.lowercased().components(separatedBy: "+"), key = parts.last ?? "", want = Set(parts.dropLast())
        let mods = event.modifiers
        if want.contains("mod") != mods.contains(.command) { return false }
        if want.contains("ctrl") != mods.contains(.control) { return false }
        if want.contains("alt") != mods.contains(.option) { return false }
        if want.contains("shift"), !mods.contains(.shift) { return false }
        let letterOrDigit = key.utf8.count == 1 && key.utf8.allSatisfy { (97...122).contains($0) || (48...57).contains($0) }
        if !want.contains("shift"), mods.contains(.shift), letterOrDigit || ["enter", "backspace", "delete", "up", "down", "tab"].contains(key) { return false }
        let named = ["up":"arrowup", "down":"arrowdown", "left":"arrowleft", "right":"arrowright", "esc":"escape", "space":" ", "del":"delete"]
        // Keypad Enter (76) is not recordable as an accelerator, but a browser reports it as `e.key == "Enter"`.
        let physical = event.keyCode == 76 ? "enter" : codes[event.keyCode]?.lowercased() ?? ""
        let semantic = ["enter":"enter", "tab":"tab", "backspace":"backspace", "delete":"delete", "escape":"escape", "arrowup":"arrowup", "arrowdown":"arrowdown", "arrowleft":"arrowleft", "arrowright":"arrowright", "space":" "][physical]
        let k = semantic ?? event.characters.lowercased()
        if let alias = named[key] { return k == alias }
        if letterOrDigit { return k == key || physical == ((Int(key) != nil ? "digit" : "key") + key) }
        return k == key
    }
    // Apple virtual key codes, kept here so KudzuCore remains Foundation-only.
    public static let codes: [UInt16: String] = [
        0:"KeyA",1:"KeyS",2:"KeyD",3:"KeyF",4:"KeyH",5:"KeyG",6:"KeyZ",7:"KeyX",8:"KeyC",9:"KeyV",11:"KeyB",12:"KeyQ",13:"KeyW",14:"KeyE",15:"KeyR",16:"KeyY",17:"KeyT",
        18:"Digit1",19:"Digit2",20:"Digit3",21:"Digit4",22:"Digit6",23:"Digit5",24:"Equal",25:"Digit9",26:"Digit7",27:"Minus",28:"Digit8",29:"Digit0",30:"BracketRight",31:"KeyO",32:"KeyU",33:"BracketLeft",34:"KeyI",35:"KeyP",36:"Enter",37:"KeyL",38:"KeyJ",39:"Quote",40:"KeyK",41:"Semicolon",42:"Backslash",43:"Comma",44:"Slash",45:"KeyN",46:"KeyM",47:"Period",48:"Tab",49:"Space",50:"Backquote",51:"Backspace",53:"Escape",
        64:"F17",79:"F18",80:"F19",90:"F20",96:"F5",97:"F6",98:"F7",99:"F3",100:"F8",101:"F9",103:"F11",109:"F10",111:"F12",105:"F13",107:"F14",113:"F15",106:"F16",118:"F4",120:"F2",122:"F1",117:"Delete",123:"ArrowLeft",124:"ArrowRight",125:"ArrowDown",126:"ArrowUp"
    ]
}
