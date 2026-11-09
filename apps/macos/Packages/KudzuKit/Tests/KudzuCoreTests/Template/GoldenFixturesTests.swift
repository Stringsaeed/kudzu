import Foundation
import Testing
@testable import KudzuCore

func loadT1Fixture<T: Decodable>(_ name: String, as: T.Type) throws -> T {
    let url = try #require(Bundle.module.url(forResource: name, withExtension: nil, subdirectory: "Fixtures"))
    return try JSONDecoder().decode(T.self, from: Data(contentsOf: url))
}
func fixtureTime(_ now: String) throws -> (Date, TimeZone, Int) {
    let regex = try NSRegularExpression(pattern: #"^([+-]?\d{4,})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d+))?(Z|[+-]\d{2}:\d{2})$"#)
    let match = try #require(regex.firstMatch(in:now,range:NSRange(location:0,length:now.utf16.count)))
    func group(_ n:Int) -> String { Range(match.range(at:n),in:now).map { String(now[$0]) } ?? "" }
    let zone=group(8), sign=zone.first == "-" ? -1 : 1
    let seconds=zone == "Z" ? 0 : sign * ((Int(zone.dropFirst().prefix(2)) ?? 0) * 3600 + (Int(zone.suffix(2)) ?? 0) * 60)
    let fraction=group(7), nanos=Int((fraction + String(repeating:"0",count:9)).prefix(9)) ?? 0
    let day=GregorianDate.days(Int(group(1))!,Int(group(2))!,Int(group(3))!)
    let whole=day * 86400 + Int(group(4))! * 3600 + Int(group(5))! * 60 + Int(group(6))! - seconds
    return (Date(timeIntervalSince1970:Double(whole) + Double(nanos) / 1_000_000_000),try #require(TimeZone(secondsFromGMT:seconds)),nanos)
}
struct TemplateGoldenCase: Decodable, Sendable {
    var source: String
    var now: String
    var locale: String
    var clipboard: String?
    var formValues: [String: String]
    var resolveMap: [String: String]
    var resolveFormatting: [String: Formatting]?
    var rootSnippetID: UUID?
    var formatting: Formatting?
    var expected: RenderResult
}
struct ICUGoldenCase: Decodable { var now: String; var locale: String; var pattern: String; var expected: String }

@Test func templateGoldenFixtures() throws {
    for c in try loadT1Fixture("template.json", as: [TemplateGoldenCase].self) {
        let (date, zone, ns) = try fixtureTime(c.now)
        let context = RenderContext(now: date, timeZone: zone, locale: Locale(identifier: c.locale), clipboard: c.clipboard, formValues: c.formValues, resolveSnippet: { c.resolveMap[$0.uuidString.lowercased()] }, resolveFormatting: { c.resolveFormatting?[$0.uuidString.lowercased()] }, rootSnippetID: c.rootSnippetID, nanosecond: ns)
        #expect(Template.renderFormatted(c.source, formatting: c.formatting, context: context) == c.expected, "\(c.locale): \(c.source)")
    }
}
@Test func icuGoldenFixtures() throws {
    for c in try loadT1Fixture("template-icu.json", as: [ICUGoldenCase].self) {
        let (date, zone, ns) = try fixtureTime(c.now)
        #expect(Template.formatICU(date, pattern: c.pattern, locale: c.locale, timeZone: zone, nanosecond: ns) == c.expected, "\(c.now) \(c.locale) \(c.pattern)")
    }
}

private struct MatcherGoldenCase: Decodable {
    struct Step: Decodable {
        struct Event: Decodable {
            var kind: String
            var snippetID: UUID?
            var trigger: String?
            var typedTrigger: String?
            var deleteCount: Int
            var deleteCountIncludingSeparator: Int?
            var separator: String?
            var insertion: InsertionMethod?
            var caseStyle: CaseStyle?
            var fireMode: FireMode?
            var retype: String?
        }
        var key: String
        var event: Event?
        var buffer: String
    }
    var snippets: [Snippet]
    var bundleID: String
    var insertedCount: Int
    var steps: [Step]
}
@Test func matchingGoldenFixtures() throws {
    for c in try loadT1Fixture("matching.json", as: [MatcherGoldenCase].self) {
        var m = ExpansionMatcher(index: TriggerIndex(snippets: c.snippets, groups: [], config: MatchingConfig()))
        for step in c.steps {
            let input: KeyInput = step.key == "RESET" ? .reset : step.key == "BACKSPACE" ? .backspace : .character(step.key)
            let actual = m.handle(input, bundleID: c.bundleID)
            switch actual {
            case .expand(let match):
                let e = try #require(step.event)
                #expect(e.kind == "expand" && e.snippetID == match.snippetID && e.trigger == match.trigger && e.typedTrigger == match.typedTrigger)
                #expect(e.deleteCount == match.deleteCount && e.deleteCountIncludingSeparator == match.deleteCountIncludingSeparator && e.separator == match.separator)
                #expect(e.insertion == match.insertion && e.caseStyle == match.caseStyle && e.fireMode == match.fireMode)
                m.didExpand(match: match, insertedCount: c.insertedCount)
            case .undo(let undo):
                #expect(step.event?.kind == "undo" && step.event?.deleteCount == undo.deleteCount && step.event?.retype == undo.retype)
            case nil: #expect(step.event == nil)
            }
            #expect(m.buffer.string == step.buffer)
        }
    }
}
