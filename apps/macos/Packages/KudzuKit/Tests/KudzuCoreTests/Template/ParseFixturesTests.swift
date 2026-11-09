import Foundation
import Testing
@testable import KudzuCore

// Replays `template-parse.json` (export_fixtures.rs, template section): for every source the
// parser's nodes and diagnostics, the canonical serialization, the typed view of each tag (which
// covers `DateOffset.parse`), the static analysis, nested form fields and `splitOptions`.

private struct ParseGoldenCase: Decodable {
    struct Part: Decodable, Equatable {
        var unit: String
        var amount: Int
        init(unit: String, amount: Int) { self.unit = unit; self.amount = amount }
        init(from decoder: any Decoder) throws {
            var c = try decoder.unkeyedContainer()
            unit = try c.decode(String.self)
            amount = try c.decode(Int.self)
        }
    }
    struct Typed: Decodable, Equatable {
        var kind: String
        var format: String?
        var offset: [Part]?
        var locale: String?
        var trim: Bool?
        var id: UUID?
        var name: String?
        var `default`: String?
        var multiline: Bool?
        var options: [String]?
    }
    struct Info: Decodable {
        var formFields: [FormFieldSpec]
        var referencedSnippetIds: [UUID]
        var hasCursor: Bool
        var usesClipboard: Bool
    }
    var source: String
    var parsed: ParsedTemplate
    var serialized: String
    var typed: [Typed]
    var info: Info
    var formFields: [FormFieldSpec]
    var splitOptions: [String]
}

private func typedFixture(_ tag: TemplateTag) -> ParseGoldenCase.Typed {
    typealias T = ParseGoldenCase.Typed
    func parts(_ offset: DateOffset?) -> [ParseGoldenCase.Part]? { offset?.parts.map { .init(unit: $0.unit, amount: $0.amount) } }
    switch tag {
    case .date(let format, let offset, let locale): return T(kind: "date", format: format, offset: parts(offset), locale: locale)
    case .time(let format, let offset, let locale): return T(kind: "time", format: format, offset: parts(offset), locale: locale)
    case .clipboard(let trim): return T(kind: "clipboard", trim: trim)
    case .cursor: return T(kind: "cursor")
    case .snippet(let id): return T(kind: "snippet", id: id)
    case .field(let name, let value, let multiline): return T(kind: "field", name: name, default: value, multiline: multiline)
    case .choice(let name, let options, let value): return T(kind: "choice", name: name, default: value, options: options)
    case .unknown: return T(kind: "unknown")
    }
}

@Test func templateParseGoldenFixtures() throws {
    let resolveMap = [
        UUID(uuidString: "00000000-0000-0000-0000-000000000001"): "nested {{cursor}}{{field name=who}}",
        UUID(uuidString: "00000000-0000-0000-0000-000000000002"): "{{snippet id=\"00000000-0000-0000-0000-000000000002\"}}",
        UUID(uuidString: "00000000-0000-0000-0000-000000000003"): "never",
    ]
    let cases = try loadT1Fixture("template-parse.json", as: [ParseGoldenCase].self)
    #expect(cases.count > 100)
    for c in cases {
        let parsed = Template.parse(c.source)
        #expect(parsed == c.parsed, "parse: \(c.source.debugDescription)")
        #expect(Template.serialize(parsed.nodes).utf8.elementsEqual(c.serialized.utf8), "serialize: \(c.source.debugDescription)")
        let typed = parsed.nodes.compactMap { node -> ParseGoldenCase.Typed? in
            if case .tag(let tag) = node { return typedFixture(Template.typed(tag)) }
            return nil
        }
        #expect(typed == c.typed, "typed: \(c.source.debugDescription)")
        let info = Template.analyze(parsed)
        #expect(info.formFields == c.info.formFields && info.referencedSnippetIDs == c.info.referencedSnippetIds
                && info.hasCursor == c.info.hasCursor && info.usesClipboard == c.info.usesClipboard, "analyze: \(c.source.debugDescription)")
        #expect(Template.formFields(c.source, resolveSnippet: { resolveMap[$0] ?? nil }) == c.formFields, "formFields: \(c.source.debugDescription)")
        #expect(Template.splitOptions(c.source) == c.splitOptions, "splitOptions: \(c.source.debugDescription)")
    }
}
