import Foundation
import Testing
@testable import KudzuCore

// Rust compares strings by Unicode scalars (UTF-8 bytes). Swift `==`, `Dictionary` keys,
// `hasPrefix`, and `CharacterSet.whitespacesAndNewlines` do not. These cases are the ones
// `kudzu-core` handles and the Swift port must match. Trigger matching stays NFC on purpose.

@Suite struct V1bScalarIdentityTests {
    let composed = "é"          // U+00E9
    let decomposed = "e\u{0301}" // e + U+0301
    let id = UUID(uuidString:"00000000-0000-0000-0000-0000000000aa")!

    func yamlFile(_ text: String) throws -> (URL, ImportResult) {
        let dir = try t2Temp()
        let file = dir.appendingPathComponent("base.yml")
        try Data(text.utf8).write(to:file)
        return (dir, Espanso.importFiles([file]))
    }

    @Test func dedupeDoesNotMergeGroupsThatDifferByAZeroWidthSpace() {
        // Rust `str::trim` keeps U+200B. `CharacterSet.whitespacesAndNewlines` strips it.
        let kept = Group(name:"\u{200b}Work"), plain = Group(name:"Work")
        let library = Dedupe.dedupe(Library(groups:[kept,plain])).library
        #expect(library.groups.count == 2)
        #expect(library.groups.contains { $0.name.isExactly("\u{200b}Work") })
        #expect(library.groups.contains { $0.name.isExactly("Work") })
    }

    @Test func dedupeKeepsUsageRowsWhoseDeviceIdsDifferOnlyByNormalization() {
        let rows = [
            UsageRecord(snippetID:id,deviceID:composed,count:1),
            UsageRecord(snippetID:id,deviceID:decomposed,count:4),
        ]
        let library = Dedupe.dedupe(Library(usage:rows)).library
        #expect(library.usage.count == 2)
        #expect(library.usage.contains { $0.deviceID.isExactly(composed) && $0.count == 1 })
        #expect(library.usage.contains { $0.deviceID.isExactly(decomposed) && $0.count == 4 })
    }

    @Test func usageMergeTiesBreakByUtf8BytesNotCanonicalOrder() {
        // "e"+U+0301 sorts before "f" as bytes. Swift `<` compares NFC, where U+00E9 sorts after "f".
        func pair(_ device: String) -> [UsageRecord] {
            [UsageRecord(snippetID:id,deviceID:device,count:1), UsageRecord(snippetID:id,deviceID:device,count:2)]
        }
        let merges = Dedupe.plan(Library(usage:pair(decomposed)+pair("f"))).usageMerges
        #expect(merges.map(\.deviceID) == [decomposed,"f"])
    }

    @Test func mergeLibraryDoesNotCombineCanonicallyEquivalentDeviceIds() {
        let existing = Library(usage:[UsageRecord(snippetID:id,deviceID:composed,count:1,charsSaved:1)])
        let incoming = Library(usage:[UsageRecord(snippetID:id,deviceID:decomposed,count:5,charsSaved:9)])
        let usage = Coordinator.mergeLibrary(existing:existing,incoming:incoming).library.usage
        #expect(usage.count == 2)
        #expect(usage.contains { $0.deviceID.isExactly(composed) && $0.count == 1 })
        #expect(usage.contains { $0.deviceID.isExactly(decomposed) && $0.count == 5 })
    }

    @Test func warningGroupsStayDistinctAndSortByUtf8Bytes() {
        let groups = Coordinator.groupWarnings([
            ImportWarning(message:"f"),
            ImportWarning(message:decomposed),
            ImportWarning(message:composed),
            ImportWarning(message:composed),
        ])
        #expect(groups.map(\.count) == [2,1,1])
        #expect(groups.map(\.message) == [composed, decomposed, "f"])
    }

    @Test func nameFromContentKeepsZeroWidthSpaceAndTrimsOnlyTheTruncatedEnd() {
        #expect(UniqueName.nameFromContent("\u{200b}Hello",fallback:"X",limit:40).isExactly("\u{200b}Hello"))
        #expect(UniqueName.nameFromContent("\u{200b}",fallback:"Fallback",limit:40).isExactly("\u{200b}"))
        // Truncation keeps the ellipsis. Trailing spaces go; a trailing U+200B stays (`trim_end`).
        #expect(UniqueName.nameFromContent("hello  world",fallback:"X",limit:7).isExactly("hello…"))
        #expect(UniqueName.nameFromContent("hello\u{200b} world",fallback:"X",limit:6).isExactly("hello\u{200b}…"))
    }

    @Test func syncIndexKeepsRecordNamesThatDifferOnlyByNormalization() {
        var index = Sync.SyncIndex()
        Sync.recordSaved(index:&index,record:.init(kind:.usage,name:"usage-"+composed,payload:"{}"))
        Sync.recordSaved(index:&index,record:.init(kind:.usage,name:"usage-"+decomposed,payload:"{}"))
        #expect(index.records.count == 2)
        #expect(index.records.keys.contains { $0.isExactly("usage-"+composed) })
        #expect(index.records.keys.contains { $0.isExactly("usage-"+decomposed) })
    }

    @Test func aCombiningMarkAfterTheRecordPrefixIsNotASnippetOrGroupId() {
        let snippetID = UUID(uuidString:"00000000-0000-0000-0000-0000000000ab")!
        let groupID = UUID(uuidString:"00000000-0000-0000-0000-0000000000ac")!
        var library = Library(groups:[Group(id:groupID,name:"G")],snippets:[Snippet(id:snippetID,name:"S",triggers:[";a"],content:"c",createdAt:Date(timeIntervalSince1970:1))])
        var index = Sync.SyncIndex()
        _ = Sync.applyRemote(library:&library,index:&index,records:[],deleted:["snippet-\u{0301}"+snippetID.lowercased,"group-\u{0301}"+groupID.lowercased],deviceID:"mac")
        #expect(library.snippets.count == 1)
        #expect(library.groups.count == 1)
    }

    @Test func usageEncodeKeepsOnlyTheExactDevice() throws {
        let library = Library(usage:[
            UsageRecord(snippetID:id,deviceID:composed,count:1),
            UsageRecord(snippetID:id,deviceID:decomposed,count:4),
        ])
        let record = try #require(Sync.encode(Sync.usageName(composed),library:library,index:.init(),deviceID:composed))
        let object = try #require(JSONSerialization.jsonObject(with:Data(record.payload.utf8)) as? NSDictionary)
        let rows = try #require(object["rows"] as? [Any])
        #expect(rows.count == 1)
        #expect((object["deviceID"] as? String)?.isExactly(composed) == true)
    }

    @Test func aCanonicallyEquivalentUsageNameIsNotThisDevicesRecord() {
        var index = Sync.SyncIndex()
        Sync.recordSaved(index:&index,record:.init(kind:.usage,name:"usage-"+composed,payload:"{\"deviceID\":\"\(composed)\",\"rows\":[]}"))
        let changes = Sync.localChanges(library:.empty,index:index,deviceID:decomposed)
        #expect(changes.saves == ["meta"])
    }

    @Test func unknownPayloadKeysThatDifferOnlyByNormalizationBothSurvive() throws {
        let snippetID = UUID(uuidString:"00000000-0000-0000-0000-0000000000ad")!
        let snippet = Snippet(id:snippetID,name:"N",triggers:[";n"],content:"old",createdAt:Date(timeIntervalSince1970:1_700_000_000))
        var library = Library(snippets:[snippet])
        var index = Sync.SyncIndex()
        let name = Sync.snippetName(snippetID)
        let extra = NSMutableDictionary()
        extra.setObject(1,forKey:composed as NSString)
        extra.setObject(2,forKey:decomposed as NSString)
        let payload = String(decoding:try JSONSerialization.data(withJSONObject:extra,options:[.sortedKeys]),as:UTF8.self)
        let stored = try #require(JSONSerialization.jsonObject(with:Data(payload.utf8)) as? NSDictionary)
        #expect(stored.count == 2)
        Sync.recordSaved(index:&index,record:.init(kind:.snippet,name:name,payload:payload))
        library.snippets[0].content = "new"
        let sent = try #require(Sync.encode(name,library:library,index:index,deviceID:"mac"))
        let parsed = try #require(JSONSerialization.jsonObject(with:Data(sent.payload.utf8)) as? NSDictionary)
        let keys = parsed.allKeys.compactMap { $0 as? String }
        #expect(keys.contains { $0.isExactly(composed) })
        #expect(keys.contains { $0.isExactly(decomposed) })
        #expect(parsed["content"] as? String == "new")
    }

    @Test func espansoTriggersAndNestedMatchesUseScalarIdentity() throws {
        let yaml = """
        matches:
          - trigger: "\(composed)"
            replace: A
          - trigger: "\(decomposed)"
            replace: B
          - trigger: ":go"
            replace: "{{m}}"
            vars:
              - name: m
                type: match
                params:
                  trigger: "\(decomposed)"
        """
        let (dir, result) = try yamlFile(yaml)
        defer { try? FileManager.default.removeItem(at:dir) }
        let snippets = result.library.snippets
        let first = try #require(snippets.first { $0.triggers.contains { $0.isExactly(composed) } })
        let second = try #require(snippets.first { $0.triggers.contains { $0.isExactly(decomposed) } })
        let go = try #require(snippets.first { $0.triggers == [":go"] })
        #expect(first.id != second.id)
        #expect(go.content.contains(second.id.uuidString))
        #expect(!go.content.contains(first.id.uuidString))
    }

    @Test func espansoVariableOpenerIsTwoScalarsEvenWhenTheSecondCombines() throws {
        // Rust walks `char`s, so `{{` matches. Swift `Character`s fuse `{` with U+0301.
        let yaml = """
        matches:
          - trigger: ":v"
            replace: "{{\u{0301}}}"
        """
        let (dir, result) = try yamlFile(yaml)
        defer { try? FileManager.default.removeItem(at:dir) }
        let content = try #require(result.library.snippets.first?.content)
        // `String.contains` is grapheme-aligned, so `\{{` plus U+0301 is not a hit. Rust searches scalars.
        #expect(RustText.contains(content, "\\{{\u{0301}}}"))
        #expect(result.warnings.contains { $0.message.contains("Undefined variable") })
    }

    @Test func espansoFormFieldsResolveByScalarKey() throws {
        let yaml = """
        matches:
          - trigger: ":f"
            form: "[[\(decomposed)]]"
            form_fields:
              "\(composed)":
                default: AAA
              "\(decomposed)":
                default: BBB
        """
        let (dir, result) = try yamlFile(yaml)
        defer { try? FileManager.default.removeItem(at:dir) }
        let content = try #require(result.library.snippets.first?.content)
        #expect(content.contains("BBB"))
        #expect(!content.contains("AAA"))
    }

    @Test func espansoFieldNamesKeepAZeroWidthSpace() throws {
        // Rust `trim_matches([' ', '\t'])` keeps U+200B. `CharacterSet.whitespaces` strips it.
        let yaml = """
        matches:
          - trigger: ":f"
            form: "[[\u{200b}name]]"
            form_fields:
              "\u{200b}name":
                default: ZZZ
              name:
                default: NO
        """
        let (dir, result) = try yamlFile(yaml)
        defer { try? FileManager.default.removeItem(at:dir) }
        let content = try #require(result.library.snippets.first?.content)
        #expect(content.contains("ZZZ"))
        #expect(!content.contains("NO"))
    }

    @Test func espansoExportKeepsScalarPathsInsertionOrderAndRustSanitize() {
        let mark = Group(name:"_\u{0301}x")
        let slash = Group(name:"/\u{0301}a")
        let zwsp = Group(name:"\u{200b}Work")
        let composedGroup = Group(name:composed)
        let decomposedGroup = Group(name:decomposed)
        func snippet(_ group: Group, _ trigger: String) -> Snippet {
            Snippet(name:trigger,triggers:[trigger],content:"x",groupID:group.id)
        }
        let exported = Espanso.export(Library(
            groups:[mark,slash,zwsp,composedGroup,decomposedGroup],
            snippets:[snippet(mark,";m"),snippet(slash,";s"),snippet(zwsp,";z"),snippet(composedGroup,";c"),snippet(decomposedGroup,";d")]
        ))
        #expect(exported.keys.contains { $0.isExactly("\u{0301}x.yml") })
        #expect(exported.keys.contains { $0.isExactly("\u{0301}a.yml") })
        #expect(exported.keys.contains { $0.isExactly("\u{200b}Work.yml") })
        #expect(exported.keys.filter { $0.isExactly(composed+".yml") || $0.isExactly(decomposed+".yml") }.count == 2)

        let yaml = Espanso.export(Library(snippets:[Snippet(name:"n",triggers:[";n"],content:"hello")]))["base.yml"] ?? ""
        func at(_ token: String) -> String.Index { yaml.range(of:token)?.lowerBound ?? yaml.endIndex }
        #expect(at("trigger") < at("label"))
        #expect(at("label") < at("replace"))
    }

    @Test func raycastNamesStayDistinctAndKeepZeroWidthSpace() throws {
        let json = """
        [
          {"name":"\(composed)","text":"A","keyword":";a"},
          {"name":"\(decomposed)","text":"B","keyword":";b"},
          {"name":"go","text":"See {snippet name=\\"\(decomposed)\\"}","keyword":";go"},
          {"name":"\u{200b}Work","text":"Hi","keyword":";w"}
        ]
        """
        let result = try Raycast.importJSON(json,fileName:"snippets.json")
        let snippets = result.library.snippets
        let first = try #require(snippets.first { $0.name.isExactly(composed) })
        let second = try #require(snippets.first { $0.name.isExactly(decomposed) })
        let go = try #require(snippets.first { $0.triggers == [";go"] })
        #expect(go.content.contains(second.id.uuidString))
        #expect(!go.content.contains(first.id.uuidString))
        #expect(snippets.contains { $0.name.isExactly("\u{200b}Work") })
    }

    @Test func textExpanderGroupsAndSnippetRefsUseScalarIdentity() throws {
        let csv = "abbreviation,snippet,label,group\n\(composed),Alpha,One,\(composed)\n\(decomposed),Beta %snippet:\(decomposed)%,Two,\(decomposed)\n"
        let result = try TextExpander.importData(Data(csv.utf8),fileName:"export.csv")
        #expect(result.library.groups.count == 2)
        let first = try #require(result.library.snippets.first { $0.triggers.contains { $0.isExactly(composed) } })
        let second = try #require(result.library.snippets.first { $0.triggers.contains { $0.isExactly(decomposed) } })
        #expect(second.content.contains(second.id.uuidString))
        #expect(!second.content.contains(first.id.uuidString))
        let zwsp = try TextExpander.importData(Data("abbreviation,snippet,group\n;z,Hi,\u{200b}Work\n".utf8),fileName:"z.csv")
        #expect(zwsp.library.groups.contains { $0.name.isExactly("\u{200b}Work") })
    }

    @Test func clipboardIdentityFollowsRawBytes() throws {
        let composedClip = try #require(Clipboard.classify([[(Clipboard.uti.PLAIN_TEXT,Data(composed.utf8))]]))
        let decomposedClip = try #require(Clipboard.classify([[(Clipboard.uti.PLAIN_TEXT,Data(decomposed.utf8))]]))
        #expect(composedClip.hash != decomposedClip.hash)
        var history = Clipboard.History()
        let now = Date(timeIntervalSince1970:1_700_000_000)
        _ = history.ingest(composedClip,now:now)
        _ = history.ingest(decomposedClip,now:now)
        #expect(history.entries.count == 2)
    }

    @Test func sameJSONFallsBackToScalarEqualityWhenThePayloadIsNotJSON() {
        // serde_json fails, then Rust compares the raw strings. Swift `==` folds them.
        #expect(!Sync.sameJSON(composed, decomposed))
        #expect(!Sync.sameJSON("{\"a\":\"\(composed)\"}", "{\"a\":\"\(decomposed)\"}"))
    }

    @Test func applyRemoteReplacesContentThatDiffersOnlyByNormalization() throws {
        let when = Date(timeIntervalSince1970:10)
        let later = Date(timeIntervalSince1970:20)
        let local = Snippet(id:id,name:"N",triggers:[";n"],content:composed,createdAt:when,updatedAt:when)
        let remote = Snippet(id:id,name:"N",triggers:[";n"],content:decomposed,createdAt:when,updatedAt:later)
        let payload = String(decoding:try KudzuJSON.encode(remote,pretty:false),as:UTF8.self)
        var library = Library(snippets:[local])
        var index = Sync.SyncIndex()
        _ = Sync.applyRemote(library:&library,index:&index,records:[.init(kind:.snippet,name:Sync.snippetName(id),payload:payload)],deleted:[],deviceID:"mac")
        #expect(library.snippets.first?.content.isExactly(decomposed) == true)
    }

    @Test func snippetAndGroupRecordNamesAcceptEveryUuidParseStrForm() {
        let snippetID = UUID(uuidString:"00000000-0000-0000-0000-0000000000b1")!
        let groupID = UUID(uuidString:"00000000-0000-0000-0000-0000000000b2")!
        let stay = UUID(uuidString:"00000000-0000-0000-0000-0000000000b3")!
        let simple = snippetID.uuidString.replacingOccurrences(of:"-",with:"").lowercased()
        let library = Library(
            groups:[Group(id:groupID,name:"G")],
            snippets:[
                Snippet(id:snippetID,name:"S",triggers:[";s"],content:"s",createdAt:Date(timeIntervalSince1970:1)),
                Snippet(id:stay,name:"Stay",triggers:[";k"],content:"k",createdAt:Date(timeIntervalSince1970:1)),
            ]
        )
        #expect(Sync.encode("snippet-"+simple,library:library,index:.init(),deviceID:"mac") != nil)
        #expect(Sync.encode("group-{"+groupID.lowercased+"}",library:library,index:.init(),deviceID:"mac") != nil)
        #expect(Sync.encode("snippet-urn:uuid:"+snippetID.lowercased,library:library,index:.init(),deviceID:"mac") != nil)
        #expect(Sync.encode("snippet-URN:uuid:"+snippetID.lowercased,library:library,index:.init(),deviceID:"mac") == nil)
        var edited = library
        var index = Sync.SyncIndex()
        _ = Sync.applyRemote(
            library:&edited,index:&index,records:[],
            deleted:["snippet-"+simple,"group-{"+groupID.lowercased+"}","snippet-URN:uuid:"+stay.lowercased],
            deviceID:"mac"
        )
        #expect(edited.snippets.contains { $0.id == snippetID } == false)
        #expect(edited.groups.isEmpty)
        #expect(edited.snippets.contains { $0.id == stay })
    }

    @Test func espansoLabelTrimKeepsAZeroWidthSpace() throws {
        let named = """
        matches:
          - trigger: ":n"
            label: "\u{200b}Hello"
            replace: hi
        """
        let (dir, result) = try yamlFile(named)
        defer { try? FileManager.default.removeItem(at:dir) }
        #expect(result.library.snippets.first?.name.isExactly("\u{200b}Hello") == true)
        let blank = """
        matches:
          - trigger: ":z"
            label: "\u{200b}"
            replace: hi
        """
        let (dir2, only) = try yamlFile(blank)
        defer { try? FileManager.default.removeItem(at:dir2) }
        #expect(only.library.snippets.first?.name.isExactly("\u{200b}") == true)
    }

    @Test func espansoChoiceValuesTrimOnlySpaceAndTabAndSplitOnTheNewlineScalar() throws {
        let yaml = """
        matches:
          - trigger: ":c"
            form: "[[pick]]"
            form_fields:
              pick:
                type: choice
                values: "\u{200b}keep\\n\u{00a0}nbsp"
          - trigger: ":d"
            form: "[[line]]"
            form_fields:
              line:
                type: choice
                values: "a\\n\u{0301}b"
        """
        let (dir, result) = try yamlFile(yaml)
        defer { try? FileManager.default.removeItem(at:dir) }
        let keep = try #require(result.library.snippets.first { $0.triggers == [":c"] }?.content)
        let line = try #require(result.library.snippets.first { $0.triggers == [":d"] }?.content)
        #expect(RustText.contains(keep, "\u{200b}keep"))
        #expect(RustText.contains(keep, "\u{00a0}nbsp"))
        #expect(RustText.contains(line, "\u{0301}b"))
        #expect(!RustText.contains(line, "a\n"))
    }

    @Test func espansoFormReferenceSplitsOnTheDotScalar() throws {
        let yaml = """
        matches:
          - trigger: ":f"
            replace: "{{box.\u{0301}x}}"
            vars:
              - name: box
                type: form
                params:
                  fields:
                    \u{0301}x:
                      default: ZZZ
        """
        let (dir, result) = try yamlFile(yaml)
        defer { try? FileManager.default.removeItem(at:dir) }
        let content = try #require(result.library.snippets.first?.content)
        #expect(RustText.contains(content, "ZZZ"))
    }

    @Test func espansoQuotedScalarKeepsACombiningMarkAfterTheQuote() throws {
        let yaml = """
        matches:
          - trigger: ":q"
            replace: "\u{0301}Hi"
        """
        let (dir, result) = try yamlFile(yaml)
        defer { try? FileManager.default.removeItem(at:dir) }
        #expect(result.library.snippets.first?.content.isExactly("\u{0301}Hi") == true)
    }

    @Test func espansoDiscoverAndImportsUseScalarPrefixes() throws {
        let dir = try t2Temp()
        defer { try? FileManager.default.removeItem(at:dir) }
        let secret = "matches:\n  - trigger: \":no\"\n    replace: no\n"
        try Data(secret.utf8).write(to:dir.appendingPathComponent(".\u{0301}secret.yml"))
        try Data(secret.utf8).write(to:dir.appendingPathComponent("_\u{0301}secret.yml"))
        try Data("matches:\n  - trigger: \":ok\"\n    replace: hi\n".utf8).write(to:dir.appendingPathComponent("ok.yml"))
        let discovered = try Espanso.importDir(dir)
        #expect(discovered.library.snippets.count == 1)
        #expect(discovered.library.snippets.first?.triggers == [":ok"])

        let child = dir.appendingPathComponent("\u{0301}child")
        try FileManager.default.createDirectory(at:child,withIntermediateDirectories:true)
        try Data("matches:\n  - trigger: \":c\"\n    replace: c\n".utf8).write(to:child.appendingPathComponent("ok.yml"))
        let nested = try Espanso.importDir(dir)
        #expect(nested.library.groups.contains { $0.name.isExactly("\u{0301}child") })

        let relative = dir.appendingPathComponent("\u{0301}nope.yml")
        try Data("matches:\n  - trigger: \":rel\"\n    replace: rel\n".utf8).write(to:relative)
        let main = dir.appendingPathComponent("main.yml")
        try Data("imports:\n  - /\u{0301}nope.yml\nmatches:\n  - trigger: \":main\"\n    replace: main\n".utf8).write(to:main)
        let imported = Espanso.importFiles([main])
        #expect(imported.library.snippets.contains { $0.triggers == [":main"] })
        #expect(!imported.library.snippets.contains { $0.triggers == [":rel"] })
    }

    @Test func yamlValuesAndSingleQuotesCompareAndEscapeByScalar() {
        #expect(YAMLValue.string(composed) != YAMLValue.string(decomposed))
        var left = YAMLMap(), right = YAMLMap()
        left.append(composed,.string("A"))
        right.append(decomposed,.string("A"))
        #expect(left != right)
        #expect(Espanso.scalarYAML(.string("'\u{0301}")).isExactly("'''\u{0301}'"))
    }

    @Test func textExpanderLabelsDetectionAndFillTagsUseScalars() throws {
        let csv = Data("abbreviation,snippet,label\n;a,Hi,\u{200b}Name\n".utf8)
        let named = try TextExpander.importData(csv,fileName:"a.csv")
        #expect(named.library.snippets.first?.name.isExactly("\u{200b}Name") == true)

        let error = try #require(throws: InterchangeError.self) {
            try TextExpander.importData(Data("<?xml\u{0301}".utf8),fileName:"x.textexpander")
        }
        #expect(RustText.contains(error.errorDescription ?? "", "Couldn't read this TextExpander file"))

        let fill = try TextExpander.importData(Data("abbreviation,snippet\n;f,%filltext:name=\u{0301}Ada%\n;p,%fillpopup:name=pick:default=\(composed):\(decomposed)%\n".utf8),fileName:"f.csv")
        let field = try #require(fill.library.snippets.first { $0.triggers == [";f"] }?.content)
        let popup = try #require(fill.library.snippets.first { $0.triggers == [";p"] }?.content)
        #expect(RustText.contains(field, "\u{0301}Ada"))
        #expect(RustText.contains(popup, composed+"|"))

        let plist = """
        <?xml version="1.0" encoding="UTF-8"?>
        <plist version="1.0"><dict><key>snippetsTE2</key><array><dict>
        <key>abbreviation</key><string>;d</string>
        <key>plainText</key><string>Hi</string>
        <key>creationDate</key><date>\u{200b}2020-01-02T03:04:05Z</date>
        </dict></array></dict></plist>
        """
        let dated = try TextExpander.importData(Data(plist.utf8),fileName:"d.textexpander")
        #expect((dated.library.snippets.first?.createdAt.timeIntervalSince1970 ?? 0) > 1_700_000_000)
    }

    @Test func raycastModifiersOffsetsAndOptionsFollowChars() throws {
        let json = """
        [
          {"name":"t","text":"{clipboard|\u{200b}trim}","keyword":";t"},
          {"name":"u","text":"{clipboard|trim|\u{0301}x}","keyword":";u"},
          {"name":"o","text":"{clipboard offset=\u{200b}0}","keyword":";o"},
          {"name":"a","text":"{argument options=a,\u{0301}b}","keyword":";a"},
          {"name":"l","text":"{date locale=en-\u{0301}US}","keyword":";l"}
        ]
        """
        let result = try Raycast.importJSON(json,fileName:"s.json")
        func content(_ trigger: String) throws -> String {
            try #require(result.library.snippets.first { $0.triggers == [trigger] }?.content)
        }
        let trimmed = try content(";t")
        let split = try content(";u")
        let offset = try content(";o")
        #expect(clipboardTrim(trimmed) == false)
        #expect(result.warnings.contains { $0.message.contains("Modifiers") && $0.trigger == ";t" })
        #expect(clipboardTrim(split) == true)
        #expect(result.warnings.contains { $0.message.contains("Modifiers") && $0.trigger == ";u" })
        #expect(clipboardTrim(offset) == nil)
        #expect(result.warnings.contains { $0.message.contains("clipboard history") && $0.trigger == ";o" })
        #expect(RustText.contains(try content(";a"), "a|\u{0301}b"))
        #expect(RustText.contains(try content(";l"), "en_\u{0301}US"))
    }

    @Test func kudzuOffsetTrimMatchesRustWhitespace() {
        #expect(Strftime.kudzuOffsetToSeconds("\u{200b}+1d") == nil)
        #expect(Strftime.kudzuOffsetToSeconds(" +1d") == 86400)
        #expect(Strftime.kudzuOffsetToSeconds("\u{00a0}+1d") == 86400)
    }

    @Test func xmlPlistTrimsAndKeysMatchRust() throws {
        #expect(throws: (any Error).self) { try XMLPlist.parse("<plist><integer>\u{200b}3</integer></plist>") }
        #expect(throws: (any Error).self) { try XMLPlist.parse("<plist><string\u{200b}>hi</string></plist>") }
        let dict = try XMLPlist.parse("<plist><dict><key>\(composed)</key><string>A</string><key>\(decomposed)</key><string>B</string></dict></plist>")
        #expect(dict[composed]?.text?.isExactly("A") == true)
        #expect(dict[decomposed]?.text?.isExactly("B") == true)
        let date = try XMLPlist.parse("<plist><date>\u{200b}2020-01-02T03:04:05Z</date></plist>")
        #expect(date.dateText?.isExactly("\u{200b}2020-01-02T03:04:05Z") == true)
    }

    private func clipboardTrim(_ content: String) -> Bool? {
        for case .tag(let tag) in Template.parse(content).nodes {
            if case .clipboard(let trim) = tag.typed { return trim }
        }
        return nil
    }
}
