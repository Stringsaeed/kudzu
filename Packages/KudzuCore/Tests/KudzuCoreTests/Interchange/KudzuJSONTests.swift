import Foundation
import Testing
@testable import KudzuCore

@Suite("KudzuJSON Tests")
struct KudzuJSONTests {
    @Test("Encodes pretty-printed, sorted keys, ISO-8601 dates and round-trips")
    func encodeAndDecodeRoundTrip() throws {
        let fixedDate = Date(timeIntervalSince1970: 1_762_000_000) // 2025-11-01T12:26:40Z
        let groupID = UUID(uuidString: "11111111-2222-3333-4444-555555555555")!
        let snippetID = UUID(uuidString: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")!

        let group = GroupRecord(
            id: groupID,
            name: "Work",
            parentID: nil,
            sortIndex: 0,
            isEnabled: true,
            insertion: .paste,
            rule: AppRule(mode: .allow, bundleIDs: ["com.apple.mail"], insertion: .paste)
        )
        let snippet = SnippetRecord(
            id: snippetID,
            name: "Signature",
            triggers: [";sig", ":sig"],
            content: "Best regards,\n{{cursor}}",
            groupID: groupID,
            fireMode: .wholeWord,
            caseMode: .propagate,
            insertion: .automatic,
            rule: nil,
            isFavorite: true,
            isEnabled: true,
            createdAt: fixedDate,
            updatedAt: fixedDate
        )
        let usage = UsageRecord(
            snippetID: snippetID,
            deviceID: "mac-1",
            count: 42,
            charsSaved: 500,
            lastUsedAt: fixedDate
        )
        let library = KudzuLibrary(
            formatVersion: KudzuLibrary.currentFormatVersion,
            groups: [group],
            snippets: [snippet],
            usage: [usage]
        )

        let data = try KudzuJSON.encode(library)
        let jsonString = try #require(String(data: data, encoding: .utf8))

        // Verify pretty-printed & sorted keys ("formatVersion" before "groups" before "snippets" before "usage")
        #expect(jsonString.contains("\n"))
        let formatIndex = try #require(jsonString.range(of: "\"formatVersion\"")?.lowerBound)
        let groupsIndex = try #require(jsonString.range(of: "\"groups\"")?.lowerBound)
        let snippetsIndex = try #require(jsonString.range(of: "\"snippets\"")?.lowerBound)
        let usageIndex = try #require(jsonString.range(of: "\"usage\"")?.lowerBound)
        #expect(formatIndex < groupsIndex)
        #expect(groupsIndex < snippetsIndex)
        #expect(snippetsIndex < usageIndex)

        // Verify ISO-8601 date formatting
        #expect(jsonString.contains("2025-11-01T12:26:40Z"))

        let decoded = try KudzuJSON.decode(data)
        #expect(decoded == library)
    }

    @Test("Rejects formatVersion newer than KudzuLibrary.currentFormatVersion")
    func rejectsNewerFormatVersion() throws {
        let futureLibrary = KudzuLibrary(
            formatVersion: KudzuLibrary.currentFormatVersion + 1,
            groups: [],
            snippets: [],
            usage: []
        )
        let data = try KudzuJSON.encode(futureLibrary)

        #expect(throws: KudzuJSONError.unsupportedFormatVersion(
            found: KudzuLibrary.currentFormatVersion + 1,
            maxSupported: KudzuLibrary.currentFormatVersion
        )) {
            _ = try KudzuJSON.decode(data)
        }
    }
}
