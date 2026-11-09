// Ported from kudzu-core/src/agent/mod.rs
// The Model Context Protocol (MCP) server that lets AI agents use the library: list, read,
// expand, create and edit snippets, and search clipboard history.
//
// Transport-agnostic: [`handle`] takes one JSON-RPC message and returns the reply, if any.
// The app serves it on a Unix socket, and `Kudzu --mcp` bridges an agent's stdio to that
// socket (answering through [`Offline`] while Kudzu isn't running).
//
// Everything sent to an agent passes through [`crate::secrets`]: text that looks like a
// password, key or token is replaced with a `[redacted: …]` marker, and search only sees the
// redacted text, so a query can't probe for a secret either.

import Foundation

/// Transport-agnostic MCP: one JSON-RPC line in, an optional serialized response out.
/// All text and searches pass through Secrets before they reach an agent.
public enum Agent {
    public static let SKILL_URI = "kudzu://skill/SKILL.md"
    public static let PROTOCOL_VERSIONS = ["2025-11-25","2025-06-18","2025-03-26","2024-11-05"]
    public static let OFFLINE_MESSAGE = "Kudzu isn't running, or AI agent access is turned off. Open Kudzu, go to Settings ▸ AI agents, and turn on “Allow AI agents”."
    public static let SKILL: String = {
        guard let url = Bundle.module.url(forResource:"SKILL",withExtension:"md",subdirectory:"Resources"), let s = try? String(contentsOf:url,encoding:.utf8) else { return "" }; return s
    }()
    public struct Access: Equatable, Sendable {
        public var clipboard: Bool; public var write: Bool; public var triggerPrefix: String
        public init(clipboard: Bool, write: Bool, triggerPrefix: String = ";") { self.clipboard = clipboard; self.write = write; self.triggerPrefix = triggerPrefix }
        public static let all = Access(clipboard:true,write:true)
    }
    public struct RenderEnv: Sendable {
        public var now: Date; public var locale: String; public var clipboard: String?; public var timeZone: TimeZone
        public init(now: Date, locale: String, clipboard: String? = nil, timeZone: TimeZone = .current) { self.now = now; self.locale = locale; self.clipboard = clipboard; self.timeZone = timeZone }
    }
    public struct Failure: Error, LocalizedError, Equatable {
        public var message: String
        public init(_ message: String) { self.message = message }
        public var errorDescription: String? { message }
    }
    /// App boundary: edits run transactionally against the live library and save on success.
    public protocol Host {
        func access() throws -> Access
        func library() throws -> Library
        func editLibrary(_ edit: (inout Library) throws -> [String:Any]) throws -> [String:Any]
        func clipboard() throws -> [ClipEntry]
        func clipboardText(_ id: UUID) throws -> String?
        func renderEnv() -> RenderEnv
    }
    /// Offline bridge still advertises tools; calls explain how to enable the running app.
    public struct Offline: Host {
        public init() {}
        public func access() throws -> Access { throw Failure(OFFLINE_MESSAGE) }
        public func library() throws -> Library { throw Failure(OFFLINE_MESSAGE) }
        public func editLibrary(_ edit: (inout Library) throws -> [String:Any]) throws -> [String:Any] { throw Failure(OFFLINE_MESSAGE) }
        public func clipboard() throws -> [ClipEntry] { throw Failure(OFFLINE_MESSAGE) }
        public func clipboardText(_ id: UUID) throws -> String? { throw Failure(OFFLINE_MESSAGE) }
        public func renderEnv() -> RenderEnv { RenderEnv(now:Date(),locale:"en_US") }
    }
    public static func definitions(_ access: Access) -> [[String:Any]] {
        (try? requireDefinitions(access)) ?? []
    }
    /// The bundled tool catalog, filtered by `access`. Throws when the catalog is missing or empty.
    /// A filtered access list may be shorter than the catalog; that is not a missing catalog.
    static func requireDefinitions(_ access: Access, data: Data? = nil) throws -> [[String:Any]] {
        let bytes: Data
        if let data { bytes = data }
        else {
            guard let url = Bundle.module.url(forResource:"AgentTools",withExtension:"json",subdirectory:"Resources"), let loaded = try? Data(contentsOf:url) else {
                throw Failure("Kudzu's tool list failed to load.")
            }
            bytes = loaded
        }
        guard var tools = (try? JSONSerialization.jsonObject(with:bytes)) as? [[String:Any]], !tools.isEmpty else {
            throw Failure("Kudzu's tool list failed to load.")
        }
        tools = tools.filter { t in
            let name = t["name"] as? String ?? ""
            return (access.write || !["create_snippet","update_snippet"].contains(name)) && (access.clipboard || !["list_clipboard","get_clipboard_item"].contains(name))
        }
        for i in tools.indices { if let s = tools[i]["description"] as? String { tools[i]["description"] = s.replacingOccurrences(of:"{prefix}",with:access.triggerPrefix) } }
        return tools
    }
    /// Whether [`handle`] answers `message` (requests and malformed input do; notifications and
    /// responses don't). The bridge uses it to know when to wait for a reply.
    public static func expectsReply(_ message: String) -> Bool {
        guard let m = Sync.parsed(message) as? [String:Any] else { return true }
        return m["method"] != nil ? m["id"] != nil : !isResponse(m)
    }
    static func isResponse(_ m: [String:Any]) -> Bool { m["id"] != nil && (m["result"] != nil || m["error"] != nil) }
    static func error(_ id: Any, _ code: Int, _ message: String) -> [String:Any] { ["jsonrpc":"2.0","id":id,"error":["code":code,"message":message]] }
    /// Requests and malformed input receive replies; notifications and responses do not.
    public static func handle(host: any Host, serverVersion: String, message: String, toolData: Data? = nil) -> String? {
        guard let value = Sync.parsed(message) else { return json(error(NSNull(),-32700,"Parse error")) }
        guard let m = value as? [String:Any] else { return json(error(NSNull(),-32600,"Invalid request: expected one JSON-RPC object")) }
        guard let method = m["method"] else { return isResponse(m) ? nil : json(error(m["id"] ?? NSNull(),-32600,"Invalid request")) }
        guard let method = method as? String else { return m["id"].flatMap { json(error($0,-32600,"Invalid request: method must be a string")) } }
        let params = m["params"] as? [String:Any] ?? [:], id = m["id"]
        var result: [String:Any]
        do {
            switch method {
            case "initialize":
                let requested = params["protocolVersion"] as? String ?? "", version = PROTOCOL_VERSIONS.contains(requested) ? requested : PROTOCOL_VERSIONS[0]
                result = ["protocolVersion":version,"capabilities":["tools":["listChanged":false],"resources":["subscribe":false,"listChanged":false]],"serverInfo":["name":"kudzu","title":"Kudzu","version":serverVersion],"instructions":INSTRUCTIONS]
            case "ping": result = [:]
            case "tools/list": result = ["tools":try requireDefinitions((try? host.access()) ?? .all, data: toolData)]
            case "tools/call": result = try call(host,params)
            case "resources/list": result = ["resources":[["uri":SKILL_URI,"name":"kudzu-skill","title":"Kudzu skill","description":"How and when to use Kudzu's tools: finding and expanding snippets, writing triggers and templates, clipboard history, and redacted secrets.","mimeType":"text/markdown","size":SKILL.utf8.count]]]
            case "resources/read":
                guard let uri = params["uri"] as? String else { throw RPCError(code:-32602,message:"Missing resource uri") }
                guard uri == SKILL_URI else { throw RPCError(code:-32002,message:"Resource not found: \(uri)") }
                result = ["contents":[["uri":SKILL_URI,"mimeType":"text/markdown","text":SKILL]]]
            case "resources/templates/list": result = ["resourceTemplates":[]]
            case "prompts/list": result = ["prompts":[]]
            default: guard id != nil else { return nil }; throw RPCError(code:-32601,message:"Method not found: \(method)")
            }
            guard let id else { return nil }; return json(["jsonrpc":"2.0","id":id,"result":result])
        } catch let e as RPCError { return id.flatMap { json(error($0,e.code,e.message)) } }
        catch { return id.flatMap { json(Self.error($0,-32603,error.localizedDescription)) } }
    }
    struct RPCError: Error { var code: Int; var message: String }
    static let read = ["list_snippets","get_snippet","expand_snippet","list_groups"], write = ["create_snippet","update_snippet"], clips = ["list_clipboard","get_clipboard_item"]
    static func call(_ host: any Host, _ params: [String:Any]) throws -> [String:Any] {
        guard let name = params["name"] as? String else { throw RPCError(code:-32602,message:"Missing tool name") }
        guard (read+write+clips).contains(name) else { throw RPCError(code:-32602,message:"Unknown tool: \(name)") }
        let args: [String:Any]
        if params["arguments"] == nil || params["arguments"] is NSNull { args = [:] }
        else if let value = params["arguments"] as? [String:Any] { args = value }
        else { throw RPCError(code:-32602,message:"Tool arguments must be an object") }
        do {
            let access = try host.access()
            if write.contains(name) && !access.write { throw Failure("Creating and editing snippets is turned off in Kudzu (Settings ▸ AI agents).") }
            if clips.contains(name) && !access.clipboard { throw Failure("Clipboard history isn't shared with agents. The user can turn it on in Kudzu (Settings ▸ AI agents).") }
            let value: [String:Any]
            switch name {
            case "list_snippets": value = try listSnippets(host.library(),args,access)
            case "get_snippet": value = try getSnippet(host.library(),args)
            case "expand_snippet": value = try expandSnippet(host.library(),args,host.renderEnv())
            case "list_groups": value = try listGroups(host.library())
            case "create_snippet": value = try host.editLibrary { try createSnippet(&$0,args,Date()) }
            case "update_snippet": value = try host.editLibrary { try updateSnippet(&$0,args,Date()) }
            case "list_clipboard": value = try listClipboard(host.clipboard(),args)
            default: value = try getClipboardItem(host,args)
            }
            let text = try JSONValue(value).pretty()
            return ["content":[["type":"text","text":text]],"structuredContent":value,"isError":false]
        } catch { return ["content":[["type":"text","text":error.localizedDescription]],"isError":true] }
    }
}
