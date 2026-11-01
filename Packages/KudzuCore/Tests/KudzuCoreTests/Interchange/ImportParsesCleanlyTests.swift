import Foundation
import Testing
@testable import KudzuCore

/// Cross-layer check: everything the Espanso importer emits must be valid Kudzu template source.
@Suite struct ImportParsesCleanlyTests {
    @Test func fixtureImportsParseWithoutErrors() throws {
        let root = try #require(Bundle.module.url(forResource: "Fixtures/espanso", withExtension: nil))
        let result = try EspansoImporter.importDirectory(root)
        #expect(!result.library.snippets.isEmpty)
        for snippet in result.library.snippets {
            let errors = TemplateParser.parse(snippet.content).diagnostics.filter { $0.severity == .error }
            #expect(errors.isEmpty, "\(snippet.triggers): \(snippet.content) → \(errors.map(\.message))")
        }
    }

    @Test func literalBackslashesSurviveImportAndRender() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let match = dir.appendingPathComponent("match")
        try FileManager.default.createDirectory(at: match, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let yaml = #"""
        matches:
          - trigger: ";path"
            replace: 'C:\Users\me \\server'
          - trigger: ";bs"
            replace: 'ends with backslash \'
        """#
        try yaml.write(to: match.appendingPathComponent("base.yml"), atomically: true, encoding: .utf8)

        let library = try EspansoImporter.importDirectory(dir).library
        let context = RenderContext(now: .now, calendar: .current, timeZone: .gmt, locale: Locale(identifier: "en_US_POSIX"))
        func render(_ trigger: String) throws -> String {
            let snippet = try #require(library.snippets.first { $0.triggers.contains(trigger) })
            return TemplateRenderer.render(snippet.content, context: context).text
        }
        #expect(try render(";path") == #"C:\Users\me \\server"#)
        #expect(try render(";bs") == #"ends with backslash \"#)

        // Export back to Espanso and re-import: content must be unchanged.
        let exported = EspansoExporter.export(library)
        let again = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: again) }
        for (path, yaml) in exported {
            let url = again.appendingPathComponent("match").appendingPathComponent(path)
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try yaml.write(to: url, atomically: true, encoding: .utf8)
        }
        let reimported = try EspansoImporter.importDirectory(again).library
        #expect(Set(reimported.snippets.map(\.content)) == Set(library.snippets.map(\.content)))
    }
}
