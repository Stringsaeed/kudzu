import Testing
@testable import KudzuCore

@Suite("KeyBuffer")
struct KeyBufferTests {
    func appendsCharactersInOrder() {
        var buffer = KeyBuffer()
        buffer.apply(.character("a"))
        buffer.apply(.character("b"))
        #expect(buffer.string == "ab")
        #expect(buffer.count == 2)
    }

    func backspaceDropsLastCharacter() {
        var buffer = KeyBuffer()
        buffer.apply(.character("a"))
        buffer.apply(.character("b"))
        buffer.apply(.backspace)
        #expect(buffer.string == "a")
    }

    func backspaceOnEmptyBufferIsNoOp() {
        var buffer = KeyBuffer()
        buffer.apply(.backspace)
        #expect(buffer.isEmpty)
    }

    func resetClearsEverything() {
        var buffer = KeyBuffer()
        buffer.apply(.character("a"))
        buffer.apply(.reset)
        #expect(buffer.isEmpty)
    }

    func capsAtCapacityKeepingNewestCharacters() {
        var buffer = KeyBuffer(capacity: 4)
        for ch in "abcdef" { buffer.apply(.character(String(ch))) }
        #expect(buffer.string == "cdef")
        #expect(buffer.count == 4)
    }

    func defaultCapacityIs64() {
        var buffer = KeyBuffer()
        for _ in 0..<200 { buffer.apply(.character("x")) }
        #expect(buffer.count == 64)
        #expect(KeyBuffer.defaultCapacity == 64)
    }

    func multiScalarInputIsOneGraphemeCluster() {
        var buffer = KeyBuffer()
        buffer.apply(.character("👍🏽"))
        buffer.apply(.character("e\u{301}"))
        #expect(buffer.count == 2)
        buffer.apply(.backspace)
        #expect(buffer.count == 1)
    }

    func combiningMarkTypedSeparatelyJoinsPreviousCharacter() {
        var buffer = KeyBuffer()
        buffer.apply(.character("e"))
        buffer.apply(.character("\u{301}"))
        #expect(buffer.count == 1)
        #expect(buffer.characters.first == "é")
    }

    func regionalIndicatorsPairIntoFlags() {
        var buffer = KeyBuffer()
        buffer.apply(.character("🇺"))
        buffer.apply(.character("🇸"))
        #expect(buffer.count == 1)
        buffer.apply(.character("🇬"))
        #expect(buffer.count == 2)
    }

    func emptyStringInputIsIgnored() {
        var buffer = KeyBuffer()
        buffer.apply(.character(""))
        #expect(buffer.isEmpty)
    }

    func copiesAreIndependent() {
        var original = KeyBuffer()
        original.apply(.character("a"))
        var copy = original
        copy.apply(.character("b"))
        #expect(original.string == "a")
        #expect(copy.string == "ab")
    }
}
