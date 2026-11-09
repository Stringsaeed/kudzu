import Foundation
import Testing
@testable import KudzuCore

/// Review regressions: places where the first port followed Foundation/grapheme semantics instead of
/// the Rust bytes and chars (or the TS UTF-16 code units). Rust answers come from `kudzu-core`.
@Suite struct ClipboardReviewTests {
    func text(_ s: String) -> [Clipboard.PasteItem] { [[(Clipboard.uti.PLAIN_TEXT, Data(s.utf8))]] }

    @Test func utf8TextKeepsItsBOM() throws {
        // `String(data:encoding:.utf8)` drops a BOM; Rust keeps it, and the hash covers it.
        let c = try #require(Clipboard.classify(text("\u{feff}hello")))
        #expect(c.text == "\u{feff}hello")
        #expect(String(format: "%016llx", c.hash) == "048ddc32e0ca42ba")
        let blob = Clipboard.encodeItems([[("\u{feff}t", Data([1]))]])
        #expect(Clipboard.decodeItems(blob)?.first?.first?.0 == "\u{feff}t")
        #expect(Clipboard.decodeItems(blob)?.first?.first?.0.utf8.count == 4)
    }
    @Test func zeroWidthSpaceIsNotWhitespace() {
        // `CharacterSet.whitespacesAndNewlines` has U+200B; Rust `trim` doesn't.
        #expect(Clipboard.classify(text("\u{200b}"))?.kind == .text)
    }
    @Test func fileURLsDecodeLikeRust() {
        #expect(Clipboard.fileURLToPath("file:///a%A\u{e9}") == nil)               // `from_utf8(..).ok()?`
        #expect(Clipboard.fileURLToPath("file:///a%-0b") == "/a%-0b")             // `u8::from_str_radix("-0")` fails
        #expect(Clipboard.fileURLToPath("file:///a%+41") == "/a\u{04}1")        // window is `+4`; `from_str_radix("+4", 16)` is 4
        #expect(Clipboard.fileURLToPath("file://localhost/\u{301}x") == "/\u{301}x")
    }
    @Test func linksAndCodeUseBytesNotGraphemes() {
        #expect(Clipboard.isLink("www.\u{301}a.b"))
        #expect(Clipboard.looksLikeCode("git \u{301}status"))
        #expect(!Clipboard.looksLikeCode("x = 1; \u{301}\ny = 2; \u{301}\nz = 3; \u{301}"))
        // Combining marks aren't `Alphabetic`, so decomposed words don't count as prose.
        #expect(Clipboard.looksLikeCode("cafe\u{301} au lait;\nbonjour mon ami;\nau revoir mes amis;"))
        #expect(Clipboard.imageMimeForPath("a.PNG") == "image/png")
    }
    @Test func lineCountKeepsBareCarriageReturn() {
        var h = Clipboard.History()
        let e = h.ingest(Clipboard.classify(text("a\r\nb\r"))!, now: Date()).entry
        #expect(e.lineCount == 2)
        #expect(RustText.lines("a\r") == ["a\r"])
        #expect(RustText.lines("a\rb\r\n\r") == ["a\rb", "\r"])
    }
    @Test func restoreIsStableForEqualTimes() {
        var h = Clipboard.History(), now = Date(timeIntervalSince1970: 1_800_000_000)
        let a = h.ingest(Clipboard.classify(text("a"))!, now: now).entry, b = h.ingest(Clipboard.classify(text("b"))!, now: now).entry
        let gone = h.remove([a.id, b.id])
        h.restore(gone)
        #expect(h.entries.map(\.id) == gone.map(\.id))
    }

    // MARK: UI helpers (TS semantics)

    @Test func queriesUseJavaScriptStrings() {
        #expect(Clipboard.parseQuery("app:\u{301}mail").app == "\u{301}mail")
        #expect(Clipboard.parseQuery("a\u{feff}b").terms == ["a", "b"])           // JS `\s` has U+FEFF…
        #expect(Clipboard.parseQuery("a\u{85}b").terms == ["a\u{85}b"])           // …and not U+0085
        #expect(!Clipboard.includes("caf\u{e9}", "cafe\u{301}"))                    // code units, not canonical equivalence
        #expect(Clipboard.includes("cafe\u{301}", "cafe"))
        #expect(Clipboard.matchRanges("cafe\u{301}", terms: ["cafe"]) == [0..<4])
        #expect(Clipboard.wordCount("a\u{feff}b c") == 3)
    }
    @Test func pathsTrimOnlyTrailingSlashes() {
        #expect(Clipboard.basename("/a/b/\n") == "\n")
        #expect(Clipboard.dirname("/a/\u{301}b") == "/a")
        #expect(Clipboard.basename("/") == "")
    }
    @Test func linkTitlesFollowWHATWG() {
        func entry(_ t: String) -> ClipEntry { var e = ClipEntry(kind: .link, hash: "0", firstCopiedAt: Date(), copiedAt: Date(), copyCount: 1); e.text = t; return e }
        #expect(Clipboard.describe(entry("https://a.com:443/x/")) == (title: "a.com", subtitle: "/x"))
        #expect(Clipboard.describe(entry("https://a.com:8443/x")) == (title: "a.com:8443", subtitle: "/x"))
        #expect(Clipboard.describe(entry("MAILTO:me@a.com")).subtitle == "Email address")
        #expect(Clipboard.describe(entry("https://a.com/%E0%A4%A")) == (title: "https://a.com/%E0%A4%A", subtitle: "https://a.com/%E0%A4%A"))
        #expect(Clipboard.describe(entry("https://a.com/caf%C3%A9")).subtitle == "/caf\u{e9}")
        #expect(Clipboard.describe(entry("https://a.com/a%2Fb")).subtitle == "/a%2Fb")
    }
    @Test func colorNumbersUseToFixed() {
        #expect(Clipboard.toFixed(0.125, 2) == "0.13")   // printf gives "0.12"
        #expect(Clipboard.toFixed(0.25, 1) == "0.3")
        #expect(Clipboard.toFixed(-0.0, 3) == "0.000")
        #expect(Clipboard.toFixed(1.005, 2) == "1.00")   // 1.005 is below the tie in binary
        #expect(Clipboard.colorFormats("rgb(1e 2 3)").values[0].value == "#000000")
    }
}
