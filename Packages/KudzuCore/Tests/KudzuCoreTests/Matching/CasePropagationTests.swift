import Testing
@testable import KudzuCore

@Suite("CasePropagation")
struct CasePropagationTests {
    @Test(arguments: [
        (";sig", CaseStyle.asIs),
        (";Sig", .capitalized),
        (";SIG", .upper),
        (";sIg", .asIs),
        (";SiG", .capitalized),
        ("sig", .asIs),
        ("Sig", .capitalized),
        ("SIG", .upper),
        (";S", .capitalized),
        (";s", .asIs),
        (";SIG1", .upper),
        (";sig1", .asIs),
        (";1sig", .asIs),
        (";1Sig", .capitalized),
        (";12", .asIs),
        (";", .asIs),
        ("", .asIs),
        (";Éa", .capitalized),
        (";ÉA", .upper),
        (";éa", .asIs),
        (";ß", .asIs),
        (";Straße", .capitalized),
        (";STRASSE", .upper),
        ("你好", .asIs),
        ("AB你", .upper),
        ("A_B", .upper),
    ])
    func detectsStyle(typed: String, expected: CaseStyle) {
        #expect(CasePropagation.detect(typedTrigger: typed) == expected)
    }

    @Test(arguments: [
        (CaseStyle.asIs, "best regards", "best regards"),
        (.upper, "best regards", "BEST REGARDS"),
        (.capitalized, "best regards", "Best regards"),
        (.capitalized, "best Regards", "Best Regards"),
        (.capitalized, "BEST", "BEST"),
        (.capitalized, "", ""),
        (.upper, "", ""),
        (.capitalized, "  hello", "  Hello"),
        (.capitalized, "\"hello\"", "\"Hello\""),
        (.capitalized, "42 things", "42 things"),
        (.capitalized, "éclair", "Éclair"),
        (.upper, "straße", "STRASSE"),
        (.upper, "été", "ÉTÉ"),
        (.capitalized, "line one\nline two", "Line one\nline two"),
        (.upper, "a1;b", "A1;B"),
    ])
    func appliesStyle(style: CaseStyle, text: String, expected: String) {
        #expect(CasePropagation.apply(style, to: text) == expected)
    }
}
