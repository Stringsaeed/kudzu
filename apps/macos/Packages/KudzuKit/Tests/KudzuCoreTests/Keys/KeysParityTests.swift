import Testing
@testable import KudzuCore

// keys.ts edge cases.

@Test func singleCodeUnitKeysAreUppercased() {
    // `k.length === 1` counts UTF-16 code units, so a decomposed letter or an emoji is shown as written.
    #expect(Keys.keyLabels("mod+e\u{301}") == ["⌘", "e\u{301}"])
    #expect(Keys.keyLabels("mod+é") == ["⌘", "É"])
    #expect(Keys.acceleratorLabels("Alt+e\u{301}") == ["⌥", "e\u{301}"])
}

@Test func keypadEnterMatchesEnter() {
    // A browser reports keypad Enter as `e.key == "Enter"`.
    #expect(KeyEvent(keyCode: 76, characters: "\u{3}").matches(combo: "enter"))
    #expect(KeyEvent(keyCode: 76, characters: "\u{3}", modifiers: .command).matches(combo: "mod+enter"))
}
