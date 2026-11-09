import Testing
@testable import KudzuCore

@Test func keyLabelsAndAcceleratorLabels() {
    #expect(Keys.keyLabels("mod+shift+k") == ["⌘","⇧","K"])
    #expect(Keys.keyLabels("ctrl+alt+up") == ["⌃","⌥","↑"])
    #expect(Keys.acceleratorLabels("Control+Alt+Space") == ["⌃","⌥","Space"])
    #expect(Keys.acceleratorLabels("CommandOrControl+Shift+K") == ["⌘","⇧","K"])
}
@Test func acceleratorRecordingAndCarbonParsing() {
    #expect(Keys.acceleratorFromEvent(keyCode:49,modifiers:[.control,.option]) == "Control+Alt+Space")
    #expect(Keys.acceleratorFromEvent(keyCode:0,modifiers:[]) == nil)
    #expect(Keys.acceleratorFromEvent(keyCode:0,modifiers:.shift) == nil)
    #expect(Keys.acceleratorFromEvent(keyCode:56,modifiers:.shift) == nil)
    #expect(Keys.acceleratorFromEvent(keyCode:122,modifiers:[]) == "F1")
    #expect(Keys.acceleratorFromEvent(keyCode:53,modifiers:.command) == nil)
    #expect(Keys.parseAccelerator("Control+Alt+Space")?.carbonKeyCode == 49)
    #expect(Keys.parseAccelerator("Control+Alt+Space")?.carbonModifiers == 6144)
    #expect(Keys.parseAccelerator("CommandOrControl+Shift+K")?.carbonModifiers == 768)
    #expect(Keys.parseAccelerator("Bogus+Space") == nil)
}
@Test func shortcutsRespectModifiersSymbolsAndPhysicalLetters() {
    #expect(KeyEvent(keyCode:40,characters:"k",modifiers:.command).matches(combo:"mod+k"))
    #expect(!KeyEvent(keyCode:40,characters:"K",modifiers:[.command,.shift]).matches(combo:"mod+k"))
    #expect(KeyEvent(keyCode:44,characters:"?",modifiers:[.command,.shift]).matches(combo:"mod+?"))
    #expect(KeyEvent(keyCode:126,modifiers:.option).matches(combo:"alt+up"))
    #expect(KeyEvent(keyCode:40,characters:"ن",modifiers:.command).matches(combo:"mod+k"))
    #expect(!KeyEvent(keyCode:40,characters:"k",modifiers:[.command,.control]).matches(combo:"mod+k"))
}
