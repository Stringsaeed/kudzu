import Foundation
import Testing
@testable import KudzuCore

private func tag(_ source: String) -> KudzuCore.Tag {
    guard case .chip(let tag)? = TemplateSegments.parse(source).first else { fatalError("not a tag: \(source)") }
    return tag
}

@Test(arguments: [
    ("{{date}}", "calendar", "Date · yyyy-MM-dd"),
    ("{{date format=\"MMMM d, yyyy\" offset=+3d}}", "calendar", "Date · Long date +3d"),
    ("{{date format=\"QQQ\"}}", "calendar", "Date · QQQ"),
    ("{{time}}", "clock", "Time · HH:mm"),
    ("{{clipboard}}", "doc.on.clipboard", "Clipboard"),
    ("{{clipboard trim=true}}", "doc.on.clipboard", "Clipboard · trimmed"),
    ("{{cursor}}", "character.cursor.ibeam", "Cursor"),
    ("{{field name=email}}", "character.textbox", "Field · email"),
    ("{{choice name=tone options=\"a|b\"}}", "list.bullet", "Choice · tone"),
])
func labels(source: String, symbol: String, text: String) {
    let label = ChipLabel(tag: tag(source))
    #expect(label.symbolName == symbol)
    #expect(label.text == text)
    #expect(!label.isWarning)
    #expect(!label.accessibilityLabel.isEmpty)
}

@Test func snippetLabels() {
    let id = UUID()
    let source = "{{snippet id=\"\(id)\"}}"
    let named = ChipLabel(tag: tag(source)) { $0 == id ? "Signature" : nil }
    #expect(named.text == "Signature" && !named.isWarning && named.symbolName == "text.badge.plus")
    let missing = ChipLabel(tag: tag(source))
    #expect(missing.text == "Missing snippet" && missing.isWarning)
}

@Test func warningLabels() {
    #expect(ChipLabel(tag: tag("{{date offset=bogus}}")).isWarning)
    #expect(ChipLabel(tag: tag("{{choice name=x}}")).isWarning)
    #expect(ChipLabel(tag: KudzuCore.Tag(name: "future")).kind == .unknown)
}

@Test func settingAttributesPreservesOrderAndUnknowns() {
    let original = tag("{{date format=\"EEEE\" extra=1 locale=fr_FR}}")
    let edited = original.setting("format", string: "HH").setting("offset", string: "+1d").setting("locale", string: nil)
    #expect(edited.attributes.map(\.name) == ["format", "extra", "offset"])
    #expect(edited.source == "{{date format=\"HH\" extra=1.0 offset=\"+1d\"}}")
    #expect(tag("{{clipboard trim=true}}").setting("trim", flag: false).source == "{{clipboard}}")
    #expect(original.stringAttribute("locale") == "fr_FR" && !original.boolAttribute("locale"))
}

@Test(arguments: [[String](), ["a"], ["a|b", "c"], ["", "x", ""], ["one", "two", "three"]])
func choiceOptionsRoundTrip(options: [String]) {
    let encoded = ChoiceOptions.encode(options)
    let expected = options == [""] ? [] : options
    #expect(ChoiceOptions.decode(encoded) == expected)
    let chip = KudzuCore.Tag(name: "choice").setting("name", string: "x").setting("options", string: encoded)
    #expect(tag(chip.source).typed == .choice(name: "x", options: expected, default: nil))
}

@Test func presets() {
    #expect(DateFormatPreset.matching("EEEE")?.title == "Weekday")
    #expect(DateFormatPreset.matching("nope") == nil)
    #expect(DateFormatPreset.defaultFormat(forTagNamed: "time") == "HH:mm")
    #expect(Set(DateFormatPreset.all.map(\.id)).count == DateFormatPreset.all.count)
}
