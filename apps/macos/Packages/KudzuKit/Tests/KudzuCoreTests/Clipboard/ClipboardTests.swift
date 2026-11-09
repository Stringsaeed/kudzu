import Foundation
import Testing
@testable import KudzuCore

@Suite struct ClipboardTests {
    func text(_ s: String) -> [Clipboard.PasteItem] { [[(Clipboard.uti.PLAIN_TEXT,Data(s.utf8))]] }
    func at(_ hour: Int) -> Date { Date(timeIntervalSince1970:1_793_491_200+Double(hour)*3600) }
    func classified(_ s: String) throws -> Clipboard.Classified { try #require(Clipboard.classify(text(s))) }
    @Test func classifiesTextLinksColorsAndCode() throws {
        for (s,k): (String,ClipKind) in [("Hello there",.text),("https://kudzu.app/docs?x=1",.link),("  mailto:me@example.com ",.link),("www.example.org",.link),("#1E90FF",.color),("oklch(0.7 0.1 250)",.color),("rgb(255, 0, 0)",.color),("git commit -m \"fix\"",.code),("function add(a, b) {\n  const sum = a + b;\n  return sum;\n}",.code),("Dear team,\nThanks for the meeting today.\nSee you tomorrow.",.text),("#hashtag",.text),("https://",.text),("see https://a.com now",.text)] { #expect(try classified(s).kind == k) }
        #expect(try classified("#1E90FF").color == "#1e90ff")
    }
    @Test func codeAppsTipTheBalance() { #expect(Clipboard.classify(text("let x = foo();"),sourceBundle:"com.microsoft.VSCode")?.kind == .code); #expect(Clipboard.classify(text("let x = foo();"),sourceBundle:"com.apple.Notes")?.kind == .text) }
    @Test func ignoresWhitespaceAndEmpty() { #expect(Clipboard.classify(text("  \n ")) == nil); #expect(Clipboard.classify([]) == nil) }
    @Test func filesWinAndDecodePaths() throws {
        let c = try #require(Clipboard.classify([[(Clipboard.uti.FILE_URL,Data("file:///Users/me/My%20Report.pdf".utf8)),(Clipboard.uti.PLAIN_TEXT,Data("My Report.pdf".utf8))],[(Clipboard.uti.FILE_URL,Data("file://localhost/tmp/folder/".utf8))]]))
        #expect(c.kind == .file); #expect(c.files == ["/Users/me/My Report.pdf","/tmp/folder"])
    }
    @Test func loneImagesAreImagesButTextWins() throws {
        let c = try #require(Clipboard.classify([[(Clipboard.uti.TIFF,Data("tiffdata".utf8)),(Clipboard.uti.PNG,Data("pngdata".utf8))]]))
        #expect(c.kind == .image); #expect(c.imageType == Clipboard.uti.PNG)
        #expect(Clipboard.classify([[(Clipboard.uti.PLAIN_TEXT,Data("a\tb".utf8)),(Clipboard.uti.TIFF,Data("x".utf8))]])?.kind == .text)
    }
    @Test func richTextIsFlaggedAndUnknownDataIsOther() throws {
        #expect(Clipboard.classify([[(Clipboard.uti.RTF,Data("{\\rtf1}".utf8)),(Clipboard.uti.PLAIN_TEXT,Data("Bold".utf8))]])?.rich == true)
        let c = try #require(Clipboard.classify([[("com.figma.private",Data([1,2]))]])); #expect(c.kind == .other); #expect(c.size == 2)
    }
    @Test func privateMarkersAreDetected() { #expect(Clipboard.isPrivate([[(Clipboard.uti.PLAIN_TEXT,Data("hunter2".utf8)),("org.nspasteboard.ConcealedType",Data())]])); #expect(!Clipboard.isPrivate(text("hi"))) }
    @Test func repeatsFoldIntoOneEntryWithTimings() throws {
        var h = Clipboard.History(); let a = try classified("alpha"), b = try classified("beta"), first = h.ingest(a,now:at(0)).entry
        h.ingest(b,now:at(1)); let again = h.ingest(a,source:ClipSource(bundleId:"x",name:"X"),now:at(2))
        guard case .repeated(let top) = again else { Issue.record("repeat expected"); return }
        #expect(h.entries.count == 2); #expect(top.id == first.id); #expect(top.copyCount == 2); #expect(top.timings == [at(0),at(2)]); #expect(top.firstCopiedAt == at(0)); #expect(top.copiedAt == at(2)); #expect(top.source?.name == "X")
    }
    @Test func timingsAreCapped() throws {
        var h = Clipboard.History(); let a = try classified("x")
        for i in 0..<(Clipboard.MAX_TIMINGS+10) { h.ingest(a,now:at(i)) }
        #expect(h.entries[0].timings.count == Clipboard.MAX_TIMINGS); #expect(h.entries[0].copyCount == UInt32(Clipboard.MAX_TIMINGS+10)); #expect(h.entries[0].timings.last == at(Clipboard.MAX_TIMINGS+9))
    }
    @Test func detectsSecretsAndKeepsCorrections() throws {
        var h = Clipboard.History(); let pw = h.ingest(try classified("P@ssw0rd!"),now:at(0)).entry, note = h.ingest(try classified("lunch at noon"),now:at(1)).entry
        #expect(pw.secret == .password); #expect(pw.isSecret); #expect(!note.isSecret)
        #expect(h.setSecret(pw.id,secret:false)?.isSecret == false)
        let again = h.ingest(try classified("P@ssw0rd!"),now:at(2)).entry; #expect(again.id == pw.id); #expect(!again.isSecret)
        #expect(h.setSecret(note.id,secret:true)?.isSecret == true); #expect(h.setSecret(pw.id,secret:nil)?.isSecret == true)
    }
    @Test func rescansHistoryFromOlderRulesOnce() throws {
        var h = Clipboard.History(); h.ingest(try classified("token: abcd1234efgh"),now:at(0)); h.entries[0].secret = nil; h.secretRules = 0
        let changed = h.rescanSecrets(); #expect(changed); #expect(h.entries[0].secret == .credential); let again = h.rescanSecrets(); #expect(!again)
        #expect(try KudzuJSON.decode(Clipboard.History.self,from:Data(#"{"version":1,"entries":[]}"#.utf8)).secretRules == 0)
    }
    @Test func excerptKeepsLengthsOfLongText() throws {
        var h = Clipboard.History(); let long = String(repeating:"é",count:Clipboard.TEXT_EXCERPT_LIMIT+100)+"\nend", e = h.ingest(try classified(long),now:at(0)).entry
        #expect(e.text?.unicodeScalars.count == Clipboard.TEXT_EXCERPT_LIMIT); #expect(e.textLen == Clipboard.TEXT_EXCERPT_LIMIT+104); #expect(e.lineCount == 2)
    }
    @Test func retentionKeepsPinsAndRespectsForever() throws {
        var h = Clipboard.History(); for (i,s) in ["old","older","fresh"].enumerated() { h.ingest(try classified(s),now:at(i*24)) }
        let id = try #require(h.entries.first { $0.text == "old" }?.id); h.setPinned(id,pinned:true); var forever = h
        #expect(forever.prune(now:at(24*400),retentionDays:0).isEmpty)
        #expect(h.prune(now:at(24*9),retentionDays:7).map(\.text) == ["older"]); #expect(h.entries.count == 2); #expect(h.get(id)?.expiresAt(retentionDays:7) == nil)
        #expect(h.entries.first { $0.text == "fresh" }?.expiresAt(retentionDays:7) == at(48+24*7))
    }
    @Test func capsUnpinnedEntries() throws {
        var h = Clipboard.History(); for i in 0..<(Clipboard.MAX_ENTRIES+3) { h.ingest(try classified("n\(i)"),now:at(0)) }
        let gone = h.prune(now:at(1),retentionDays:0); #expect(gone.count == 3); #expect(h.entries.count == Clipboard.MAX_ENTRIES); #expect(gone[0].text == "n2")
    }
    @Test func removeRestoreClearAndUse() throws {
        var h = Clipboard.History(); for (i,s) in ["a","b","c"].enumerated() { h.ingest(try classified(s),now:at(i)) }
        let gone = h.remove([h.entries[1].id]); #expect(gone.count == 1); #expect(h.restore(gone).count == 1); #expect(h.entries.compactMap(\.text).joined() == "cba"); #expect(h.restore(gone).isEmpty)
        let id = h.entries[2].id; #expect(h.markUsed(id,now:at(10))?.useCount == 1); #expect(h.entries[0].id == id)
        h.setPinned(id,pinned:true); #expect(h.clear(includePinned:false).count == 2); #expect(h.entries.count == 1); #expect(h.clear(includePinned:true).count == 1)
    }
    @Test func blobsRoundTripAndRejectGarbage() throws {
        let items: [Clipboard.PasteItem] = [[(Clipboard.uti.PLAIN_TEXT,Data("hi".utf8)),(Clipboard.uti.RTF,Data("{\\rtf}".utf8))],[(Clipboard.uti.PNG,Data([0,1,2,255]))]], bytes = Clipboard.encodeItems(items)
        #expect(Clipboard.encodeItems(try #require(Clipboard.decodeItems(bytes))) == bytes); #expect(Clipboard.textOf(items) == "hi"); #expect(Clipboard.imageOf(items)?.mime == "image/png")
        #expect(Clipboard.decodeItems(Data("nope".utf8)) == nil); #expect(Clipboard.decodeItems(bytes.dropLast()) == nil); #expect(Clipboard.decodeItems(bytes+Data([0])) == nil); #expect(Clipboard.decodeItems(Clipboard.encodeItems([]))?.isEmpty == true)
    }
    @Test func historyJsonIsStable() throws {
        var h = Clipboard.History(); h.ingest(try classified("#fff"),image:ImageSize(width:1,height:2),now:at(0))
        let data = try KudzuJSON.encode(h); #expect(try KudzuJSON.decode(Clipboard.History.self,from:data) == h)
        let old = #"{"entries":[{"id":"6f1c2b9e-8a33-4f0e-9a51-2a7b9a5f2c11","kind":"text","hash":"00","firstCopiedAt":"2026-11-01T00:00:00Z","copiedAt":"2026-11-01T00:00:00Z","copyCount":1}]}"#
        let decoded = try KudzuJSON.decode(Clipboard.History.self,from:Data(old.utf8)); #expect(decoded.version == 1); #expect(!decoded.entries[0].pinned)
    }
    @Test func helpers() {
        #expect(Clipboard.excerpt("héllo",limit:2) == "hé"); #expect(Clipboard.imageMimeForPath("/a/B.JPG") == "image/jpeg"); #expect(Clipboard.imageMimeForPath("/a/b.txt") == nil)
        #expect(Clipboard.fnv1a([Data("ab".utf8),Data("c".utf8)]) != Clipboard.fnv1a([Data("a".utf8),Data("bc".utf8)])); #expect(Clipboard.defaultIgnoredApps().contains("com.1password.1password"))
    }
    @Test func queryRowsAndColorMath() throws {
        #expect(Clipboard.parseQuery("invoice app:Mail is:links").terms == ["invoice"]); #expect(Clipboard.parseQuery("app:Mail is:links").kind == .link)
        let red = try #require(Clipboard.cssToRGB("#ff0000")), f = Clipboard.formats(red)
        #expect(f.values[0].value == "#FF0000"); #expect(f.values[2].value == "hsl(0 100% 50%)"); #expect(f.onWhite == "4.00"); #expect(f.onBlack == "5.25")
    }
}
