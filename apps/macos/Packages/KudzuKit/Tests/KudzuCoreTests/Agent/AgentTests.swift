import Foundation
import Testing
@testable import KudzuCore

@Suite struct AgentTests {
    final class Fake: Agent.Host {
        var accessValue = Agent.Access.all, lib = Library(), clips: [ClipEntry] = [], currentClipboard: String?, saves = 0
        init() {
            let work = Group(name:"Work")
            lib.groups = [work]; lib.snippets = [Snippet(name:"Signature",triggers:[";sig"],content:"Best,\nSaeed",groupID:work.id),Snippet(name:"Wi-Fi",triggers:[";wifi"],content:"Network: Home\nPassword: Tr0ub4dor&3"),Snippet(name:"Greeting",triggers:[";hi"],content:#"Hi {{field name="who" default="there"}}, {{choice name="tone" options="thanks|cheers"}}!"#)]
            for (i,t) in ["meeting notes for monday","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","P@ssw0rd!"].enumerated() {
                var h = Clipboard.History(); let c = Clipboard.classify([[(Clipboard.uti.PLAIN_TEXT,Data(t.utf8))]])!
                clips.append(h.ingest(c,now:Date(timeIntervalSince1970:1_793_523_600+Double(i+1)*60)).entry)
            }
        }
        func access() throws -> Agent.Access { accessValue }
        func library() throws -> Library { lib }
        func editLibrary(_ edit: (inout Library) throws -> [String:Any]) throws -> [String:Any] { var copy = lib; let result = try edit(&copy); lib = copy; saves += 1; return result }
        func clipboard() throws -> [ClipEntry] { clips }
        func clipboardText(_ id: UUID) throws -> String? { clips.first { $0.id == id }?.text }
        func renderEnv() -> Agent.RenderEnv { .init(now:KudzuDate.parse("2026-11-08T12:00:00Z")!,locale:"en_US",clipboard:currentClipboard,timeZone:.gmt) }
        func snippet(_ trigger: String) -> Snippet { lib.snippets.first { $0.triggers.contains(trigger) }! }
    }
    func rpc(_ host: any Agent.Host, _ message: [String:Any]) throws -> [String:Any] {
        let input = try #require(Sync.json(message)), result = try #require(Agent.handle(host:host,serverVersion:"1.2.3",message:input)); return try #require(Sync.parsed(result) as? [String:Any])
    }
    func call(_ host: any Agent.Host, _ name: String, _ args: [String:Any] = [:]) throws -> (value: [String:Any], error: String?) {
        let r = try rpc(host,["jsonrpc":"2.0","id":7,"method":"tools/call","params":["name":name,"arguments":args]])
        let result = try #require(r["result"] as? [String:Any])
        if result["isError"] as? Bool == true { return ([:],(result["content"] as? [[String:Any]])?.first?["text"] as? String) }
        return (try #require(result["structuredContent"] as? [String:Any]),nil)
    }
    func encoded(_ v: [String:Any]) -> String { Sync.json(v) ?? "" }
    @Test func initializeNegotiatesTheProtocol() throws {
        let h = Fake(), r = try rpc(h,["id":1,"method":"initialize","params":["protocolVersion":"2025-06-18","capabilities":[:],"clientInfo":["name":"t","version":"1"]]]), result = try #require(r["result"] as? [String:Any])
        #expect(r["id"] as? Int == 1); #expect(result["protocolVersion"] as? String == "2025-06-18"); #expect((result["serverInfo"] as? [String:Any])?["version"] as? String == "1.2.3")
        let fallback = try rpc(h,["id":2,"method":"initialize","params":["protocolVersion":"1999-01-01"]]); #expect((fallback["result"] as? [String:Any])?["protocolVersion"] as? String == Agent.PROTOCOL_VERSIONS[0])
    }
    @Test func servesTheSkillAsAResource() throws {
        for host: any Agent.Host in [Fake(),Agent.Offline()] {
            let r = try rpc(host,["id":1,"method":"resources/list"]), result = try #require(r["result"] as? [String:Any]); #expect((result["resources"] as? [[String:Any]])?.first?["uri"] as? String == Agent.SKILL_URI)
            let read = try rpc(host,["id":2,"method":"resources/read","params":["uri":Agent.SKILL_URI]]), v = try #require(read["result"] as? [String:Any]); #expect((v["contents"] as? [[String:Any]])?.first?["text"] as? String == Agent.SKILL)
            #expect(Agent.SKILL.hasPrefix("---\nname: kudzu"))
            let missing = try rpc(host,["id":3,"method":"resources/read","params":["uri":"kudzu://nope"]]); #expect((missing["error"] as? [String:Any])?["code"] as? Int == -32002)
        }
    }
    @Test func skillResourceMatchesSourceBytes() throws {
        var root = URL(fileURLWithPath:#filePath).deletingLastPathComponent()
        let relative = "apps/macos/Packages/KudzuKit/Sources/KudzuCore/Resources/SKILL.md"
        while root.path != "/" && !FileManager.default.fileExists(atPath:root.appendingPathComponent(relative).path) { root.deleteLastPathComponent() }
        let bundled = try Data(contentsOf:root.appendingPathComponent(relative)); #expect(bundled == Data(Agent.SKILL.utf8))
    }
    @Test func notificationsAndResponsesGetNoReply() throws {
        let h = Fake()
        for m: [String:Any] in [["method":"notifications/initialized"],["method":"tools/list"],["id":4,"result":[:]]] { let s = try #require(Sync.json(m)); #expect(!Agent.expectsReply(s)); #expect(Agent.handle(host:h,serverVersion:"1",message:s) == nil) }
        for s in ["{nope","[1,2]",#"{"jsonrpc":"2.0","id":3,"method":"nope"}"#] { #expect(Agent.expectsReply(s)); let r = try #require(Agent.handle(host:h,serverVersion:"1",message:s)); #expect(r.contains("error")) }
        let ping = try rpc(h,["id":"p","method":"ping"]); #expect(ping["id"] as? String == "p"); #expect((ping["result"] as? [String:Any])?.isEmpty == true)
    }
    @Test func toolsFollowAccess() throws {
        #expect(try Agent.count(["limit":NSNumber(value:UInt64.max)],"limit",50,200) == 200)
        #expect(throws:(any Error).self) { try Agent.count(["limit":NSNumber(value:1.5)],"limit",50,200) }
        let h = Fake(); #expect(Agent.definitions(try h.access()).count == 8); h.accessValue = .init(clipboard:false,write:false)
        #expect(Agent.definitions(try h.access()).compactMap { $0["name"] as? String } == ["list_snippets","get_snippet","expand_snippet","list_groups"])
        #expect(try call(h,"list_clipboard").error?.contains("Settings") == true); #expect(try call(h,"create_snippet",["name":"x","content":"y","triggers":[";y"]]).error != nil); #expect(h.saves == 0)
        let unknown = try rpc(h,["id":1,"method":"tools/call","params":["name":"rm_rf"]]); #expect((unknown["error"] as? [String:Any])?["code"] as? Int == -32602)
    }
    @Test func offlineListsToolsAndExplainsCalls() throws { #expect(Agent.definitions(.all).count == 8); #expect(try call(Agent.Offline(),"list_snippets").error == Agent.OFFLINE_MESSAGE) }
    @Test func listsAndSearchesSnippetsWithoutLeakingSecrets() throws {
        let h = Fake(), all = try call(h,"list_snippets").value
        #expect(all["total"] as? Int == 3); #expect(all["triggerPrefix"] as? String == ";"); #expect(!encoded(all).contains("Tr0ub4dor")); #expect(encoded(all).contains("Network: Home Password: [redacted: password]"))
        #expect(try call(h,"list_snippets",["query":"Tr0ub4dor"]).value["total"] as? Int == 0)
        #expect(try call(h,"list_snippets",["query":"best"]).value["total"] as? Int == 1); #expect(try call(h,"list_snippets",["group":"work"]).value["total"] as? Int == 1)
    }
    @Test func readsASnippetByTrigger() throws {
        let h = Fake(), r = try call(h,"get_snippet",["trigger":";HI"]); #expect(r.error == nil); #expect(r.value["name"] as? String == "Greeting"); #expect(r.value["form"] as? Bool == true); #expect((r.value["fields"] as? [Any])?.count == 2)
        let wifi = try call(h,"get_snippet",["trigger":";wifi"]); #expect(wifi.value["content"] as? String == "Network: Home\nPassword: [redacted: password]")
        #expect(try call(h,"get_snippet",["id":"nope"]).error?.contains("isn't a snippet id") == true)
    }
    @Test func expandsWithValuesDefaultsAndAScrubbedClipboard() throws {
        let h = Fake(); #expect(try call(h,"expand_snippet",["trigger":";hi"]).value["text"] as? String == "Hi there, thanks!")
        let r = try call(h,"expand_snippet",["trigger":";hi","values":["who":"Ana","tone":"cheers","extra":"1"]]); #expect(r.value["text"] as? String == "Hi Ana, cheers!"); #expect(r.value["unknownFields"] as? [String] == ["extra"])
        #expect(try call(h,"expand_snippet",["trigger":";hi","values":["tone":"rude"]]).error?.contains("thanks, cheers") == true)
        h.lib.snippets.append(Snippet(name:"Paste",triggers:[";p"],content:"Here: {{clipboard}} ok")); h.currentClipboard = "P@ssw0rd!"
        #expect(try call(h,"expand_snippet",["trigger":";p"]).value["text"] as? String == "Here: [redacted: password] ok")
        h.lib.snippets.append(Snippet(name:"Nest",triggers:[";n"],content:"Guest info: {{snippet id=\"\(h.snippet(";wifi").id)\"}}"))
        #expect(try call(h,"expand_snippet",["trigger":";n"]).value["text"] as? String == "Guest info: Network: Home\nPassword: [redacted: password]")
    }
    @Test func createsSnippetsAndGroups() throws {
        let h = Fake(), r = try call(h,"create_snippet",["name":"Address","content":"1 Main St","triggers":[" ;addr "],"group":"Personal/Home","fireMode":"wholeWord"])
        #expect(r.error == nil); #expect(h.snippet(";addr").fireMode == .wholeWord); #expect(h.lib.groups.count == 3); #expect(h.lib.snippets[0].triggers == [";addr"])
        let phone = try call(h,"create_snippet",["name":"Phone","content":"555","triggers":[";ph"],"group":"home"])
        #expect((phone.value["created"] as? [String:Any])?["group"] as? String == "Personal/Home"); #expect(h.lib.groups.count == 3)
    }
    @Test func rejectsBadCreatesWithoutSaving() throws {
        let h = Fake()
        for (args,needle): ([String:Any],String) in [(["name":"Dup","content":"x","triggers":[";SIG"]],"already the trigger of “Signature”"),(["name":"No trigger","content":"x","triggers":[]],"at least one trigger"),(["name":"Blank","content":"x","triggers":["   "]],"only spaces"),(["name":"","content":"x","triggers":[";a"]],"name"),(["name":"Marker","content":"pw: [redacted: password]","triggers":[";m"]],"marker"),(["name":"Mode","content":"x","triggers":[";q"],"fireMode":"later"],"fireMode")] { #expect(try call(h,"create_snippet",args).error?.contains(needle) == true) }
        #expect(h.saves == 0); #expect(h.lib.snippets.count == 3)
    }
    @Test func warnsAboutShadowedTriggers() throws { let r = try call(Fake(),"create_snippet",["name":"Sign off","content":"Cheers","triggers":[";si"]]); #expect((r.value["warnings"] as? [String])?.first?.contains(";sig") == true) }
    @Test func updatesOnlyWhatIsPassedAndProtectsSecrets() throws {
        let h = Fake(), sig = h.snippet(";sig"), wifi = h.snippet(";wifi")
        let r = try call(h,"update_snippet",["id":sig.id.lowercased,"content":"Thanks,\nSaeed","favorite":true,"group":NSNull()]); #expect(r.error == nil)
        let after = h.snippet(";sig"); #expect(after.content == "Thanks,\nSaeed"); #expect(after.isFavorite); #expect(after.groupID == nil); #expect(after.name == "Signature"); #expect(after.updatedAt >= sig.updatedAt)
        #expect(try call(h,"update_snippet",["id":wifi.id.lowercased,"content":"Network: Home\nPassword: [redacted: password]"]).error?.contains("only be changed in Kudzu") == true)
        #expect(try call(h,"update_snippet",["id":wifi.id.lowercased,"triggers":[";wf"]]).error == nil); #expect(h.snippet(";wf").content == wifi.content)
        let i = h.lib.snippets.firstIndex { $0.id == sig.id }!; h.lib.snippets[i].formatting = Formatting(source:h.lib.snippets[i].content,spans:[])
        #expect(try call(h,"update_snippet",["id":sig.id.lowercased,"content":"new"]).error?.contains("formatted") == true)
        #expect(try call(h,"update_snippet",["id":sig.id.lowercased,"triggers":[";hi"]]).error?.contains("already the trigger") == true)
    }
    @Test func clipboardWithholdsSecrets() throws {
        let h = Fake(), r = try call(h,"list_clipboard"), items = try #require(r.value["items"] as? [[String:Any]])
        #expect(items.count == 3); #expect(items[0]["text"] is NSNull); #expect((items[0]["withheld"] as? String)?.contains("password") == true); #expect(items[0]["length"] == nil && items[0]["lines"] == nil)
        #expect(items[1]["text"] is NSNull); #expect(items[1]["redacted"] as? [String] == ["API key"]); #expect(items[2]["text"] as? String == "meeting notes for monday"); #expect(!encoded(r.value).contains("ghp_"))
        #expect(try call(h,"list_clipboard",["query":"ghp_"]).value["total"] as? Int == 0); #expect(try call(h,"list_clipboard",["query":"monday"]).value["total"] as? Int == 1)
        #expect(try call(h,"get_clipboard_item",["id":h.clips[1].id.lowercased]).value["text"] is NSNull)
        #expect(try call(h,"get_clipboard_item",["id":h.clips[0].id.lowercased]).value["text"] as? String == "meeting notes for monday")
    }
    @Test func clipboardFollowsTheUsersCorrections() throws {
        let h = Fake(); h.clips[2].secretOverride = false; h.clips[0].secretOverride = true
        let r = try call(h,"list_clipboard"), items = try #require(r.value["items"] as? [[String:Any]])
        #expect(items[0]["text"] as? String == "P@ssw0rd!"); #expect(items[0]["redacted"] == nil); #expect(items[2]["text"] is NSNull); #expect((items[2]["withheld"] as? String)?.contains("marked") == true)
        #expect(try call(h,"list_clipboard",["query":"monday"]).value["total"] as? Int == 0)
    }
    @Test func fieldDefaultsAndOptionsAreRedactedToo() throws {
        let h = Fake(), token = "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"
        h.lib.snippets.append(Snippet(name:"Deploy",triggers:[";dep"],content:"Key: {{field name=\"key\" default=\"\(token)\"}} via {{choice name=\"env\" options=\"dev|\(token)\"}}"))
        for name in ["get_snippet","expand_snippet","list_snippets"] { let r = try call(h,name,name == "list_snippets" ? [:] : ["trigger":";dep"]); #expect(r.error == nil); #expect(!encoded(r.value).contains("wWPw5k4a")) }
    }
}
