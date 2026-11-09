import Foundation
import Testing
@testable import KudzuCore

@Suite struct StrftimeTests {
    @Test func convertsAllSupportedSpecifiersBothWays() {
        let p = "%Y-%m-%d %H:%M:%S %y %e %I %p %A %a %B %b %j %Z %z %%", c = Strftime.toICU(p)
        #expect(c.unsupportedSpecifiers.isEmpty); #expect(c.icuFormat == "yyyy-MM-dd HH:mm:ss yy d hh a EEEE EEE MMMM MMM DDD zzz xx %"); #expect(Strftime.toStrftime(c.icuFormat) == p)
    }
    @Test func quotesLiteralLettersAndReportsUnsupported() {
        let c = Strftime.toICU("%Y-%V-%u"); #expect(c.icuFormat == "yyyy-%'V'-%'u'"); #expect(c.unsupportedSpecifiers == ["%V","%u"])
        #expect(Strftime.toICU("%A, %B %e, %Y %I:%M:%S %p (%a %b %d '%y day %j %Z %z) 100%%").icuFormat == "EEEE, MMMM d, yyyy hh:mm:ss a (EEE MMM dd ''yy 'day' DDD zzz xx) 100%")
        #expect(Strftime.toICU("50%").icuFormat == "50%")
    }
    @Test func offsetsConvertBothWays() {
        for (n,s): (Int64,String?) in [(86400,"+1d"),(-86400,"-1d"),(777600,"+1w2d"),(-14400,"-4h"),(1800,"+30min"),(5,"+1min"),(0,nil)] { #expect(Strftime.secondsToKudzuOffset(n) == s) }
        for (s,n): (String,Int64?) in [("+1d",86400),("-1w2d",-777600),("+30min",1800),("2m",60*86400),("",nil),("+",nil),("+3x",nil),("d",nil)] { #expect(Strftime.kudzuOffsetToSeconds(s) == n) }
    }
}
@Suite struct UniqueNameTests {
    @Test func numbersNamesFromTwoCaseInsensitivelyFillingGaps() {
        #expect(UniqueName.make("New Snippet",existing:["Other"]) == "New Snippet"); #expect(UniqueName.make("New Snippet",existing:["New Snippet"]) == "New Snippet 2")
        #expect(UniqueName.make("New Snippet",existing:["New Snippet","New Snippet 2","new snippet 3"]) == "New Snippet 4"); #expect(UniqueName.make("Foo",existing:["FOO"]) == "Foo 2"); #expect(UniqueName.make("A",existing:["A","A 3"]) == "A 2")
    }
    @Test func triggersNumberWithoutSpace() { #expect(UniqueName.makeTrigger(";snippet",existing:[";snippet"]) == ";snippet2"); #expect(UniqueName.makeTrigger(";snippet",existing:[";SNIPPET",";snippet2"]) == ";snippet3"); #expect(UniqueName.makeTrigger(";snippet",existing:[]) == ";snippet") }
    @Test func nameFromContentUsesFirstLineTruncatesAndFallsBack() {
        #expect(UniqueName.nameFromContent("\n\n  Hello world \nsecond",fallback:"X",limit:40) == "Hello world"); #expect(UniqueName.nameFromContent(String(repeating:"a",count:100),fallback:"X",limit:10) == String(repeating:"a",count:10)+"…"); #expect(UniqueName.nameFromContent("  \n ",fallback:"Fallback",limit:40) == "Fallback")
    }
}
@Suite struct CSVTests {
    @Test func handlesQuotesNewlinesAndCrlf() { #expect(CSV.parse("\u{feff}a,b\r\n\"x, \"\"y\"\"\",\"line1\nline2\"\r\n\r\nlast,\n") == [["a","b"],["x, \"y\"","line1\nline2"],["last",""]]) }
    @Test func keepsEmptyQuotedFieldAndUnterminatedTail() { #expect(CSV.parse("a,\"\"") == [["a",""]]); #expect(CSV.parse("a,\"open") == [["a","open"]]); #expect(CSV.parse("").isEmpty) }
    @Test func delimitersInsideQuotesDoNotCount() { #expect(CSV.parse("sig,\"Hello; how; are; you\",Signature\nb,plain,Lab") == [["sig","Hello; how; are; you","Signature"],["b","plain","Lab"]]) }
    @Test func detectsSemicolonDelimiter() { #expect(CSV.parse("a;b\nc;d") == [["a","b"],["c","d"]]) }
}
@Suite struct XMLPlistTests {
    @Test func parsesNestedValuesAndEntities() throws {
        let xml = #"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>name</key><string>Tom &amp; Jerry &#233; &#x1F600;</string><key>n</key><integer>-3</integer><key>on</key><true/><key>empty</key><string/><key>when</key><date>2020-01-02T03:04:05Z</date><key>list</key><array><string>a</string><dict><key>k</key><false/></dict></array></dict></plist>"#
        let v = try XMLPlist.parse(xml); #expect(v["name"]?.text == "Tom & Jerry é 😀"); #expect(v["n"]?.integer == -3); #expect(v["on"]?.integer == 1); #expect(v["empty"]?.text == ""); #expect(v["when"]?.dateText == "2020-01-02T03:04:05Z"); #expect(v["list"]?.items?.count == 2); #expect(v["list"]?.items?[1]["k"]?.integer == 0)
    }
    @Test func keepsWhitespaceAndCdataInsideStrings() throws { #expect(try XMLPlist.parse("<plist><dict><key>t</key><string>  a\n  <![CDATA[<b>]]> </string></dict></plist>")["t"]?.text == "  a\n  <b> ") }
    @Test func normalizesLineEndingsButKeepsCharacterReferences() throws { #expect(try XMLPlist.parse("<plist><dict><key>t</key><string>a\r\nb\rc&#13;d&#0;e</string></dict></plist>")["t"]?.text == "a\nb\nc\rd&#0;e") }
    @Test func rejectsMalformedInput() { for xml in ["<plist><dict><key>a</key>","<plist><dict><string>x</string></dict></plist>","",String(repeating:"<array>",count:200)+String(repeating:"</array>",count:200)] { #expect(throws:(any Error).self) { try XMLPlist.parse(xml) } } }
}
@Suite struct TemplateOutTests {
    @Test func escapesBracesAndBackslashesInLiteralRuns() { var out = TemplateOut(); out.text("a {"); out.text("{b} \\"); out.tag("{{cursor}}"); out.text("end"); #expect(out.finish() == "a \\{{b} \\\\{{cursor}}end") }
}
@Suite struct TextExpanderMacroTests {
    func convert(_ raw: String) -> (String,[String]) { var p: [String] = []; let out = TextExpander.convert(raw,problems:&p); return (out,p) }
    @Test func dateSpecifiersMergeIntoOnePattern() { let (out,p) = convert("%A, %B %1d, %Y at %H:%M"); #expect(p.isEmpty); #expect(out == "{{date format=\"EEEE, MMMM d, yyyy\"}} at {{date format=\"HH:mm\"}}") }
    @Test func percentSignsThatAreNotMacrosStayLiteral() { let (out,p) = convert("100% sure, 50%, %q, %"); #expect(p.isEmpty); #expect(out == "100% sure, 50%, %q, %") }
    @Test func fillMacrosBecomeFormTags() { let (out,p) = convert("Hi %filltext:name=Customer:default=Jo%, %fillarea:name=Note:width=40%, %fillpopup:name=Tone:Formal:default=Casual%"); #expect(p.isEmpty); #expect(out == #"Hi {{field name="Customer" default="Jo"}}, {{field name="Note" multiline=true}}, {{choice name="Tone" options="Casual|Formal" default="Casual"}}"#) }
    @Test func unsupportedMacrosWarnAndStayText() { let (out,p) = convert("%key:return% and %calc:1+1% and %@+1D (no date)"); #expect(p.count == 3); #expect(out == "%key:return% and %calc:1+1% and %@+1D (no date)") }
    @Test func dateMathBecomesAnOffset() { let (out,p) = convert("%@+1D%A, %@-2M%B %@+3m%H:%M"); #expect(p.isEmpty); #expect(out == #"{{date format="EEEE" offset="+1d"}}, {{date format="MMMM" offset="-2m"}} {{date format="HH:mm" offset="+3min"}}"#) }
    @Test func optionalSectionsAreAlwaysIncluded() { let (out,p) = convert("A%fillpart:name=Opt:default=yes% extra %clipboard%fillpartend%B"); #expect(out == "A extra {{clipboard}}B"); #expect(p.count == 1); #expect(convert("A%fillpart:name=Opt%B").0 == "AB") }
}
@Suite struct RaycastConversionTests {
    func convert(_ raw: String) -> (String,[String]) { var p: [String] = []; let out = Raycast.convert(raw,problems:&p); return (out,p) }
    @Test func offsetsSwapMinuteAndMonthUnits() {
        for (raw,expected): (String,String?) in [("+1d","+1d"),("-2M","-2m"),("+30m","+30min"),("1y2d","+1y2d"),("+2y +5M","+2y5m"),("-1d -2h","-1d2h"),("+1d -2h",nil),("+x",nil),("+5",nil),("",nil)] { #expect(Raycast.convertOffset(raw) == expected) }
    }
    @Test func plainBracesAreNotPlaceholders() { let (out,p) = convert(#"fn main() { println!("{x}") } {clipboardy}"#); #expect(out == #"fn main() { println!("{x}") } {clipboardy}"#); #expect(p.isEmpty) }
    @Test func backslashEscapeAppliesOnlyBeforePlaceholders() { #expect(convert(#"\{clipboard}"#).0 == "{clipboard}"); let (out,p) = convert(#"\frac\{a\}{b} C:\path\{x}"#); #expect(p.isEmpty); #expect(out == #"\\frac\\{a\\}{b} C:\\path\\{x}"#) }
    @Test func unnamedArgumentsGetSeparateFields() { #expect(convert(#"{argument} and {argument} and {argument name="Who"}"#).0 == #"{{field name="argument 1"}} and {{field name="argument 2"}} and {{field name="Who"}}"#) }
    @Test func quotedValuesMayContainBracesAndPipes() { let (out,p) = convert(#"{argument name="a}b" default="x|y"}"#); #expect(p.isEmpty); #expect(out == #"{{field name="a}b" default="x|y"}}"#) }
}

@Suite struct T2BoundaryTests {
    @Test func unicodeIdentityAndUIHelpersStayLiteral() throws {
        let composed = "\u{e9}", decomposed = "e\u{301}", now = Date(timeIntervalSince1970:1000)
        let a = Snippet(name:"a",triggers:[";x"],content:composed,createdAt:now), b = Snippet(name:"b",triggers:[";x"],content:decomposed,createdAt:now)
        #expect(Dedupe.plan(Library(snippets:[a,b])).snippetMerges.isEmpty)
        #expect(Dedupe.normalizedTriggers([composed,decomposed]).count == 2)
        #expect(UniqueName.make(composed,existing:[decomposed]).utf8.elementsEqual(composed.utf8))
        #expect(!Sync.sameJSON(#"{"text":"é"}"#,"{\"text\":\"e\u{301}\"}"))
        #expect(Strftime.toICU("%a\u{301}").icuFormat == "EEE\u{301}")
        #expect(Clipboard.formatBytes(10752) == "11 KB")
        let c = try #require(Clipboard.classify([[(Clipboard.uti.PLAIN_TEXT,Data("https://WWW.EXAMPLE.COM/a%2Fb".utf8))]]))
        var h = Clipboard.History(); let e = h.ingest(c,now:now).entry
        #expect(Clipboard.describe(e).title == "example.com"); #expect(Clipboard.describe(e).subtitle == "/a%2Fb")
        #expect(Clipboard.cssToRGB("hsl(120deg 100% 50%)") == Clipboard.RGBA(0,1,0))
    }
}
