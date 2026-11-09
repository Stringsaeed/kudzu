import Carbon.HIToolbox
import CoreGraphics
import Foundation

// Synthesized keystrokes, marked so our own tap ignores them. Port of `macos/keys.rs`.

/// Why a keystroke couldn't be posted.
enum KeyError: Error, Hashable, Sendable {
    /// No post-event permission, or Secure Input is on.
    case notAllowed
    case eventCreation
}

enum Keys {
    /// `eventSourceUserData` on events we synthesize; the tap lets them through unmatched.
    static let syntheticMarker: Int64 = 0x004B_5544_5A55
    /// `eventSourceUserData` on held physical keys we re-post; the tap matches them like real typing.
    static let replayMarker: Int64 = 0x004B_5552_504C

    static let keyV: CGKeyCode = 9
    static let keyC: CGKeyCode = 8
    static let keyDelete: CGKeyCode = 51
    static let keyLeft: CGKeyCode = 123
    static let keyRight: CGKeyCode = 124
    /// Longest Unicode string per synthetic event (apps truncate longer ones).
    static let chunkUTF16 = 20

    /// `@unchecked Sendable`: a CGEventSource is an immutable CF object once created, and
    /// CoreGraphics allows sharing it between threads (the Rust engine shares it the same way).
    private final class Source: @unchecked Sendable {
        let source: CGEventSource?
        init() {
            // A private state source, so modifiers the user is physically holding don't leak into our keys.
            source = CGEventSource(stateID: .privateState)
        }
    }

    private static let shared = Source()

    /// Whether synthetic input can reach other apps right now.
    static func canPost() -> Bool {
        CGPreflightPostEventAccess() && !IsSecureEventInputEnabled()
    }

    private static func press(_ key: CGKeyCode, flags: CGEventFlags = [], unicode: [UInt16] = []) throws(KeyError) {
        guard canPost() else { throw .notAllowed }
        for down in [true, false] {
            guard let event = CGEvent(keyboardEventSource: shared.source, virtualKey: key, keyDown: down) else {
                throw .eventCreation
            }
            event.flags = flags
            event.setIntegerValueField(.eventSourceUserData, value: syntheticMarker)
            if !unicode.isEmpty {
                unicode.withUnsafeBufferPointer { buffer in
                    event.keyboardSetUnicodeString(stringLength: buffer.count, unicodeString: buffer.baseAddress)
                }
            }
            event.post(tap: .cgSessionEventTap)
        }
    }

    private static func repeatKey(_ key: CGKeyCode, _ count: Int) throws(KeyError) {
        for _ in 0..<max(0, count) {
            try press(key)
        }
    }

    /// Presses Delete `count` times.
    static func deleteBackward(_ count: Int) throws(KeyError) {
        try repeatKey(keyDelete, count)
    }

    /// Presses Left Arrow `count` times.
    static func moveLeft(_ count: Int) throws(KeyError) {
        try repeatKey(keyLeft, count)
    }

    /// Presses Right Arrow `count` times.
    static func moveRight(_ count: Int) throws(KeyError) {
        try repeatKey(keyRight, count)
    }

    /// Presses ⌘C.
    static func copy() throws(KeyError) {
        try press(keyC, flags: .maskCommand)
    }

    /// Presses ⌘V.
    static func paste() throws(KeyError) {
        try press(keyV, flags: .maskCommand)
    }

    /// Types `text` as Unicode keystrokes, chunked on grapheme boundaries so emoji ZWJ sequences
    /// and combining marks never split across events.
    static func typeText(_ text: String) throws(KeyError) {
        for chunk in chunks(text) {
            try press(0, unicode: chunk)
        }
    }

    /// UTF-16 chunks of at most ``chunkUTF16`` units (a longer single grapheme goes alone).
    static func chunks(_ text: String) -> [[UInt16]] {
        var out: [[UInt16]] = []
        var chunk: [UInt16] = []
        for g in text {
            let units = Array(g.utf16)
            if !chunk.isEmpty && chunk.count + units.count > chunkUTF16 {
                out.append(chunk)
                chunk = []
            }
            chunk.append(contentsOf: units)
        }
        if !chunk.isEmpty {
            out.append(chunk)
        }
        return out
    }
}
