import Foundation
import Testing
@testable import KudzuCore

@Suite("Espanso Import & Export Tests")
struct EspansoInterchangeTests {
    private func fixturesRootURL() throws -> URL {
        let url = Bundle.module.url(forResource: "espanso", withExtension: nil, subdirectory: "Fixtures")
        return try #require(url)
    }

    @Test("Golden test: imports Espanso directory with groups, nested groups, imports, variables, forms, and warnings")
    func goldenDirectoryImport() throws {
        let root = try fixturesRootURL()
        let result = try EspansoImporter.importDirectory(root)
        let library = result.library

        // Verify groups:
        // Auto-loaded files in match/: base.yml, dynamic.yml, forms.yml, unsupported.yml, work/meetings.yml
        // Plus base.yml imports ./_private.yml -> group "_private"
        // _unimported.yml is prefixed with `_` and not imported -> must NOT appear
        let groupNames = library.groups.map(\.name)
        #expect(groupNames.contains("base"))
        #expect(groupNames.contains("_private"))
        #expect(groupNames.contains("dynamic"))
        #expect(groupNames.contains("forms"))
        #expect(groupNames.contains("unsupported"))
        #expect(groupNames.contains("work"))
        #expect(groupNames.contains("meetings"))
        #expect(!groupNames.contains("_unimported"))

        // Verify subdirectory group parenting: `meetings` has `parentID == work.id`
        let workGroup = try #require(library.groups.first(where: { $0.name == "work" }))
        let meetingsGroup = try #require(library.groups.first(where: { $0.name == "meetings" }))
        #expect(workGroup.parentID == nil)
        #expect(meetingsGroup.parentID == workGroup.id)

        // Helper to look up snippet by primary trigger
        let byTrigger: [String: SnippetRecord] = Dictionary(
            uniqueKeysWithValues: library.snippets.map { ($0.triggers[0], $0) }
        )

        // 1. base.yml assertions
        let sig = try #require(byTrigger[":sig"])
        #expect(sig.name == "Email Signature")
        #expect(sig.content == "Best regards,\nJane Doe\n")
        #expect(sig.fireMode == .immediate)
        #expect(sig.caseMode == .exact)

        let hello = try #require(byTrigger[":hello"])
        #expect(hello.triggers == [":hello", ":hi"])
        #expect(hello.name == ":hello")
        #expect(hello.content == "Hello there!")
        #expect(hello.fireMode == .wholeWord)
        #expect(hello.caseMode == .propagate)

        let ther = try #require(byTrigger["ther"])
        #expect(ther.fireMode == .wholeWord)
        #expect(ther.caseMode == .propagate)

        let div = try #require(byTrigger[":div"])
        #expect(div.name == "HTML Div")
        #expect(div.content == "<div>{{cursor}}</div>")
        #expect(div.insertion == .type)

        let escaped = try #require(byTrigger[":escaped"])
        #expect(escaped.content == "Literal braces \\{{not_a_var}} here")

        // 2. _private.yml (via imports: in base.yml, using global_vars `company_date` from base.yml)
        let secret = try #require(byTrigger[":secret"])
        #expect(secret.name == "Imported Secret")
        #expect(secret.content == "Loaded from _private.yml on {{date format=\"yyyy-MM-dd\"}}")
        #expect(byTrigger[":unimported"] == nil)

        // 3. forms.yml assertions
        let greet = try #require(byTrigger[":greet"])
        #expect(greet.name == "Birthday Greeting")
        #expect(greet.content == "Hey {{field name=\"name\" default=\"Friend\"}}, {{field name=\"note\" default=\"Happy Birthday\" multiline=true}}!")

        let order = try #require(byTrigger[":order"])
        #expect(order.content == #"Order: {{choice name="drink" options="Coffee|Tea\\|Green|Juice" default="Coffee"}} ({{choice name="size" options="Small|Medium|Large"}})"#)

        let verboseForm = try #require(byTrigger[":verbose_form"])
        #expect(verboseForm.content == "Hi {{field name=\"user\" default=\"Admin\"}}, role: {{choice name=\"role\" options=\"Developer|Designer\" default=\"Developer\"}}")

        // 4. dynamic.yml assertions
        let now = try #require(byTrigger[":now"])
        #expect(now.content == "It's {{date format=\"HH:mm\"}}")

        let tomorrow = try #require(byTrigger[":tomorrow"])
        #expect(tomorrow.content == "Tomorrow is {{date format=\"yyyy-MM-dd\" offset=\"+1d\" locale=\"en_US\"}}")

        let fullDate = try #require(byTrigger[":full_date"])
        #expect(fullDate.content == "{{date format=\"EEEE, MMMM d, yyyy hh:mm:ss a (EEE MMM dd ''yy 'day' DDD zzz xx) 100%\"}}")

        let pasteLink = try #require(byTrigger[":paste_link"])
        #expect(pasteLink.content == "<a href=\"{{clipboard}}\">{{cursor}}</a>")

        let nestedSig = try #require(byTrigger[":nested_sig"])
        #expect(nestedSig.content == "Thanks!\n{{snippet id=\"\(sig.id.uuidString)\"}}")

        // 5. work/meetings.yml assertions
        let meeting = try #require(byTrigger[":meeting"])
        #expect(meeting.groupID == meetingsGroup.id)
        #expect(meeting.content == "Let's meet at {{cursor}} tomorrow.")
        #expect(meeting.caseMode == .propagate)

        let standup = try #require(byTrigger[":standup"])
        #expect(standup.content == "Yesterday: {{field name=\"yesterday\"}}\nToday: {{field name=\"today\"}}")

        // 6. unsupported.yml assertions & warnings
        // Regex and image_path matches are skipped
        #expect(byTrigger[":cat"] == nil)
        #expect(result.warnings.contains(where: { $0.trigger == ":greet\\((?P<person>.*)\\)" }))
        #expect(result.warnings.contains(where: { $0.trigger == ":cat" }))

        // Markdown and HTML imported as plain text with warnings
        let richMD = try #require(byTrigger[":rich_md"])
        #expect(richMD.content == "This *text* is **bold**")
        #expect(result.warnings.contains(where: { $0.trigger == ":rich_md" }))

        let richHTML = try #require(byTrigger[":rich_html"])
        #expect(richHTML.content == "<p>Hello <b>world</b></p>")
        #expect(result.warnings.contains(where: { $0.trigger == ":rich_html" }))

        // Unsupported variable types (shell, script, random, echo) kept as escaped literal text + warning
        let ip = try #require(byTrigger[":ip"])
        #expect(ip.content == "IP: \\{{output}}")
        #expect(result.warnings.contains(where: { $0.trigger == ":ip" }))

        let py = try #require(byTrigger[":py"])
        #expect(py.content == "Out: \\{{script_out}}")
        #expect(result.warnings.contains(where: { $0.trigger == ":py" }))

        let rand = try #require(byTrigger[":rand"])
        #expect(rand.content == "Quote: \\{{r}}")
        #expect(result.warnings.contains(where: { $0.trigger == ":rand" }))

        let echoTest = try #require(byTrigger[":echo_test"])
        #expect(echoTest.content == "Echo: \\{{e}}")
        #expect(result.warnings.contains(where: { $0.trigger == ":echo_test" }))

        // Unresolved nested match trigger kept as literal + warning
        let brokenNest = try #require(byTrigger[":broken_nest"])
        #expect(brokenNest.content == "Missing: \\{{missing_match}}")
        #expect(result.warnings.contains(where: { $0.trigger == ":broken_nest" }))

        // Unsupported strftime specifiers (%V, %u) warned and kept as literal in format
        let weirdDate = try #require(byTrigger[":weird_date"])
        #expect(weirdDate.content == "{{date format=\"yyyy-%'V'-%'u'\"}}")
        #expect(result.warnings.contains(where: { $0.trigger == ":weird_date" && $0.message.contains("%V") }))
    }

    @Test("Round-trip: import fixtures -> export to YAML -> import again yields equivalent snippets")
    func roundTripImportExportImport() throws {
        let root = try fixturesRootURL()
        let matchDir = root.appendingPathComponent("match", isDirectory: true)
        let filesToRoundTrip = [
            matchDir.appendingPathComponent("base.yml"),
            matchDir.appendingPathComponent("forms.yml"),
            matchDir.appendingPathComponent("dynamic.yml"),
            matchDir.appendingPathComponent("work/meetings.yml"),
        ]

        let firstImport = try EspansoImporter.importFiles(filesToRoundTrip)
        #expect(firstImport.warnings.isEmpty)

        let exportedYAMLMap = EspansoExporter.export(firstImport.library)
        #expect(exportedYAMLMap.keys.contains("base.yml"))
        #expect(exportedYAMLMap.keys.contains("forms.yml"))
        #expect(exportedYAMLMap.keys.contains("dynamic.yml"))
        #expect(exportedYAMLMap.keys.contains("work/meetings.yml"))

        // Write exported YAML files to a temporary directory and re-import
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("kudzu-espanso-roundtrip-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        for (relPath, yamlContent) in exportedYAMLMap {
            let fileURL = tempDir.appendingPathComponent(relPath)
            try FileManager.default.createDirectory(
                at: fileURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try yamlContent.write(to: fileURL, atomically: true, encoding: .utf8)
        }

        let secondImport = try EspansoImporter.importDirectory(tempDir)
        #expect(secondImport.warnings.isEmpty)
        #expect(secondImport.library.snippets.count == firstImport.library.snippets.count)

        let firstByTrigger = Dictionary(
            uniqueKeysWithValues: firstImport.library.snippets.map { ($0.triggers[0], $0) }
        )
        let secondByTrigger = Dictionary(
            uniqueKeysWithValues: secondImport.library.snippets.map { ($0.triggers[0], $0) }
        )

        for (trigger, s1) in firstByTrigger {
            let s2 = try #require(secondByTrigger[trigger], "Missing snippet for trigger \(trigger) after round-trip")
            #expect(s2.name == s1.name)
            #expect(s2.triggers == s1.triggers)
            #expect(s2.fireMode == s1.fireMode)
            #expect(s2.caseMode == s1.caseMode)
            #expect(s2.insertion == s1.insertion)

            // Normalize nested snippet UUIDs when comparing content strings
            let normalizedContent1 = normalizeSnippetIDs(in: s1.content, snippets: firstImport.library.snippets)
            let normalizedContent2 = normalizeSnippetIDs(in: s2.content, snippets: secondImport.library.snippets)
            #expect(normalizedContent2 == normalizedContent1)
        }
    }

    private func normalizeSnippetIDs(in content: String, snippets: [SnippetRecord]) -> String {
        var result = content
        for snippet in snippets {
            if let primary = snippet.triggers.first {
                result = result.replacingOccurrences(
                    of: "{{snippet id=\"\(snippet.id.uuidString)\"}}",
                    with: "{{snippet trigger=\"\(primary)\"}}"
                )
            }
        }
        return result
    }

    @Test("StrftimeConverter converts all 18 supported specifiers and offsets bidirectionally")
    func strftimeAndOffsetConversions() {
        let pattern = "%Y-%m-%d %H:%M:%S %y %e %I %p %A %a %B %b %j %Z %z %%"
        let converted = StrftimeConverter.toICU(pattern)
        #expect(converted.unsupportedSpecifiers.isEmpty)
        #expect(converted.icuFormat == "yyyy-MM-dd HH:mm:ss yy d hh a EEEE EEE MMMM MMM DDD zzz xx %")

        let backToStrftime = StrftimeConverter.toStrftime(converted.icuFormat)
        #expect(backToStrftime == pattern)

        // Offset conversions
        #expect(StrftimeConverter.secondsToKudzuOffset(86_400) == "+1d")
        #expect(StrftimeConverter.secondsToKudzuOffset(-86_400) == "-1d")
        #expect(StrftimeConverter.secondsToKudzuOffset(777_600) == "+1w2d")
        #expect(StrftimeConverter.secondsToKudzuOffset(-14_400) == "-4h")
        #expect(StrftimeConverter.secondsToKudzuOffset(1_800) == "+30min")
        #expect(StrftimeConverter.secondsToKudzuOffset(0) == nil)

        #expect(StrftimeConverter.kudzuOffsetToSeconds("+1d") == 86_400)
        #expect(StrftimeConverter.kudzuOffsetToSeconds("-1w2d") == -777_600)
        #expect(StrftimeConverter.kudzuOffsetToSeconds("+30min") == 1_800)
    }

    @Test("Preserves deterministic file and match ordering and exposes macOS default config directories")
    func deterministicOrderingAndDefaultPaths() throws {
        let root = try fixturesRootURL()
        let first = try EspansoImporter.importDirectory(root)
        let second = try EspansoImporter.importDirectory(root)

        #expect(first.library.groups.map(\.name) == second.library.groups.map(\.name))
        #expect(first.library.snippets.map(\.triggers) == second.library.snippets.map(\.triggers))
        let firstContents = first.library.snippets.map { normalizeSnippetIDs(in: $0.content, snippets: first.library.snippets) }
        let secondContents = second.library.snippets.map { normalizeSnippetIDs(in: $0.content, snippets: second.library.snippets) }
        #expect(firstContents == secondContents)

        let fakeHome = URL(fileURLWithPath: "/Users/tester", isDirectory: true)
        let defaults = EspansoImporter.defaultConfigDirectories(homeDirectory: fakeHome)
        #expect(defaults.count == 2)
        #expect(defaults[0].path == "/Users/tester/Library/Application Support/espanso")
        #expect(defaults[1].path == "/Users/tester/.config/espanso")
    }
}
