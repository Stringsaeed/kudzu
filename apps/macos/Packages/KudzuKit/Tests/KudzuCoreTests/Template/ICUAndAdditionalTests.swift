import Foundation
import Testing
@testable import KudzuCore

@Suite("ICU pattern formatting") struct ICUFormatTests {
    private func format(_ pattern: String, _ locale: String = "en_US", _ now: String = "2024-03-05T14:07:09.123456+05:30") throws -> String {
        let (date, zone, ns) = try fixtureTime(now)
        return Template.formatICU(date, pattern: pattern, locale: locale, timeZone: zone, nanosecond: ns)
    }
    @Test func numericFields() throws {
        #expect(try format("yyyy-MM-dd HH:mm:ss") == "2024-03-05 14:07:09")
        #expect(try format("y yy yyyyy M d D DDD") == "2024 24 02024 3 5 65 065")
        #expect(try format("h hh K k H a") == "2 02 2 14 14 PM")
        #expect(try format("S SS SSS SSSSSS") == "1 12 123 123456")
        #expect(try format("h k K a", "en_US", "2024-01-01T00:00:00Z") == "12 24 0 AM")
    }
    @Test func namesAndQuotes() throws {
        #expect(try format("EEEE, MMMM d") == "Tuesday, March 5")
        #expect(try format("EEE MMM EEEEE MMMMM EEEEEE", "en") == "Tue Mar T M Tu")
        #expect(try format("'Week' w', Q'Q QQQ QQQQ") == "Week 10, Q1 Q1 1st quarter")
        #expect(try format("'o''clock' ''") == "o'clock '")
        #expect(try format("G GGGG") == "AD Anno Domini")
        #expect(try format("yyyy.MM.dd' at 'HH:mm") == "2024.03.05 at 14:07")
    }
    @Test func zones() throws {
        #expect(try format("Z ZZZZ ZZZZZ", "en") == "+0530 GMT+05:30 +05:30")
        #expect(try format("x xx xxx X XXX", "en") == "+0530 +0530 +05:30 +0530 +05:30")
        #expect(try format("z zzzz O", "en") == "GMT+5:30 GMT+05:30 GMT+5:30")
        #expect(try format("X XXX x z ZZZZZ", "en", "2024-01-01T00:00:00Z") == "Z Z +00 GMT Z")
    }
    @Test func localizedNames() throws {
        for (pattern, locale, expected) in [("EEEE d MMMM","fr_FR","mardi 5 mars"),("EEEE, d. MMMM","de_DE","Dienstag, 5. März"),("EEEE d 'de' MMMM","es","martes 5 de marzo"),("MMMM","it_IT","marzo"),("EEEE","pt_BR","terça-feira"),("MMM","nl_NL","mrt"),("MMMM","ar","مارس"),("MMMM","xx_YY","March")] { #expect(try format(pattern, locale) == expected) }
    }
    @Test func localWeekdayNumbersFollowTheFirstDayOfWeek() throws {
        #expect(try format("e c") == "3 3")
        #expect(try format("e c", "de_DE") == "2 2")
        #expect(try format("eeee ccc") == "Tuesday Tue")
    }
    @Test func weekBasedYearAndUnknownLetters() throws {
        #expect(try format("YYYY-'W'ww", "en", "2021-01-01T00:00:00Z") == "2020-W53")
        #expect(try format("yyyy jj", "en", "2021-01-01T00:00:00Z") == "2021 jj")
        #expect(try format("5 / - :", "en") == "5 / - :")
    }
}
@Test func uppercaseUUIDReferencesResolve() {
    let id = UUID(), ctx = RenderContext(now: .now, resolveSnippet: { $0 == id ? "ok" : nil })
    #expect(Template.render("{{snippet id=\"\(id.uuidString)\"}}", context: ctx).text == "ok")
    let simple = id.uuidString.replacingOccurrences(of: "-", with: "")
    let source = "{{snippet id=\"\(simple)\"}}"
    #expect(Template.render(source, context: ctx).text == source)
}
@Test func nestedDiagnosticsUseNestedRanges() {
    let id = UUID(), ctx = RenderContext(now: .now, resolveSnippet: { $0 == id ? "ab{{DATE}}" : nil })
    let result = Template.render("0123456789{{snippet id=\"\(id)\"}}", context: ctx)
    #expect(result.text == "0123456789ab{{DATE}}")
    #expect(result.diagnostics[0].range == 2..<10)
}
@Test func formMemoryIsBounded() {
    var memory = FormMemory(); let ids = (0..<200).map { _ in UUID() }.sorted { $0.uuidString < $1.uuidString }
    for id in ids { memory.record([:], for: id) }
    let newcomer = UUID(); memory.record([:], for: newcomer)
    #expect(memory.values.count == 200 && memory.values[newcomer] != nil)
    #expect(memory.values[ids[0]] == nil || newcomer.uuidString < ids[0].uuidString)
}
@Test func offsetsKeepTheContextsFixedUTCOffsetAcrossDST() throws {
    let (date,_,_) = try fixtureTime("2024-03-09T12:00:00-05:00")
    let zone = try #require(TimeZone(identifier:"America/New_York"))
    let ctx = RenderContext(now:date,timeZone:zone)
    #expect(Template.render(#"{{date offset=+1d format="yyyy-MM-dd HH:mm XXX"}}"#,context:ctx).text == "2024-03-10 12:00 -05:00")
}
