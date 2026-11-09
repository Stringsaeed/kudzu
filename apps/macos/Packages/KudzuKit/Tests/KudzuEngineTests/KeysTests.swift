import CoreGraphics
import KudzuCore
@testable import KudzuEngine
import Testing

// Port of the tests in `macos/keys.rs` and `macos/tap.rs` (pure parts; no tap, no permissions).
@Suite struct KeysTests {
    @Test func chunksRespectGraphemes() {
        let family = "👨‍👩‍👧‍👦" // 11 UTF-16 units, one grapheme
        let text = "abcdefghij\(family)\(family)"
        let parts = Keys.chunks(text)
        #expect(parts.map(\.count) == [10, 11, 11])
        #expect(String(decoding: parts[1], as: UTF16.self) == family)
        #expect(Keys.chunks(String(repeating: "x", count: 45)).map(\.count) == [20, 20, 5])
        #expect(Keys.chunks("").isEmpty)
        #expect(String(decoding: parts.flatMap { $0 }, as: UTF16.self) == text)
    }

    @Test func markersMatchTheRustEngine() {
        #expect(Keys.syntheticMarker == 0x004B_5544_5A55)
        #expect(Keys.replayMarker == 0x004B_5552_504C)
    }

    private func utf16(_ s: String) -> [UInt16] { Array(s.utf16) }

    @Test func shortcutsAndNavigationReset() {
        #expect(TapEvents.classifyParts(flags: .maskCommand, key: 0, text: utf16("a")) == .reset)
        #expect(TapEvents.classifyParts(flags: .maskControl, key: 0, text: utf16("a")) == .reset)
        #expect(TapEvents.classifyParts(flags: [], key: 123, text: [0xF702]) == .reset)
        #expect(TapEvents.classifyParts(flags: [], key: 53, text: [0x1B]) == .reset)
        #expect(TapEvents.classifyParts(flags: [], key: 122, text: [0xF704]) == .reset) // F1
    }

    @Test func editingKeys() {
        #expect(TapEvents.classifyParts(flags: [], key: 51, text: [0x7F]) == .backspace)
        #expect(TapEvents.classifyParts(flags: [], key: 36, text: [0x0D]) == .character("\n"))
        #expect(TapEvents.classifyParts(flags: [], key: 48, text: [0x09]) == .character("\t"))
    }

    @Test func textKeys() {
        // Shift and Option still produce text.
        #expect(TapEvents.classifyParts(flags: CGEventFlags(rawValue: 1 << 17), key: 0, text: utf16("A")) == .character("A"))
        #expect(TapEvents.classifyParts(flags: CGEventFlags(rawValue: 1 << 19), key: 0, text: utf16("å")) == .character("å"))
        #expect(TapEvents.classifyParts(flags: [], key: 0, text: utf16("😀")) == .character("😀"))
        // Dead key: no text yet, no context change.
        #expect(TapEvents.classifyParts(flags: CGEventFlags(rawValue: 1 << 19), key: 14, text: []) == nil)
        // An unpaired surrogate is not text.
        #expect(TapEvents.classifyParts(flags: [], key: 0, text: [0xD83D]) == .reset)
    }
}
