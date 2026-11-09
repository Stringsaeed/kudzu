import Foundation
import KudzuCore
@testable import KudzuServices
import Testing

/// A library-backed host, like the app's (`AppHost` in `agent.rs`).
final class FakeAgentHost: Agent.Host, @unchecked Sendable {
    // @unchecked: `lib` is only touched while holding `lock`.
    private let lock = NSLock()
    private var lib: Library
    init(library: Library) { lib = library }
    func access() throws -> Agent.Access { Agent.Access(clipboard: false, write: true, triggerPrefix: ";") }
    func library() throws -> Library { lock.withLock { lib } }
    func editLibrary(_ edit: (inout Library) throws -> [String: Any]) throws -> [String: Any] {
        try lock.withLock { try edit(&lib) }
    }
    func clipboard() throws -> [ClipEntry] { [] }
    func clipboardText(_ id: UUID) throws -> String? { nil }
    func renderEnv() -> Agent.RenderEnv { Agent.RenderEnv(now: Date(timeIntervalSince1970: 0), locale: "en_US") }
}

final class Flag: @unchecked Sendable {
    private let lock = NSLock()
    private var on = true
    var value: Bool {
        get { lock.withLock { on } }
        set { lock.withLock { on = newValue } }
    }
}

@Suite(.serialized) struct AgentServerTests {
    @Test func shutdownFinishesBothAdmittedSocketMutations() async throws {
        let dir = tempDir("agent-drain")
        defer { try? FileManager.default.removeItem(at: dir) }
        let repository = try await LibraryRepository.open(.local(url: dir.appendingPathComponent("Library.store")))
        let gate = SocketRequestGate()
        let server = AgentServer(dir: dir, version: "1", handle: { message in
            if message.contains("first") { await gate.wait() }
            return try? await repository.handleAgent(message: message, version: "1", host: FakeAgentHost(library: .empty))
        }, allowed: { true })
        server.apply(on: true)
        defer { server.shutDown(); Task { await gate.open() } }
        let path = server.socketURL.path
        let first = Task.detached { try Self.socketCreate("first", path: path) }
        for _ in 0..<500 {
            if await gate.isWaiting { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(await gate.isWaiting)
        let second = Task.detached { try Self.socketCreate("second", path: path) }
        for _ in 0..<500 {
            if server.testingAcceptedRequests == 2 { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(server.testingAcceptedRequests == 2)
        server.shutDown()
        await gate.open()
        await server.drain()
        #expect(!(try await first.value).contains("isError\":true"))
        #expect(!(try await second.value).contains("isError\":true"))
        let reopened = try await LibraryRepository.open(.local(url: dir.appendingPathComponent("Library.store")))
        #expect(reopened.snapshot.snippets.map(\.name).sorted() == ["first", "second"])
    }

    private static func socketCreate(_ name: String, path: String) throws -> String {
        let connection = try #require(UnixConnection.connect(path))
        defer { connection.close() }
        connection.setReadTimeout(10)
        let message = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"create_snippet\",\"arguments\":{\"name\":\"\(name)\",\"content\":\"saved\",\"triggers\":[\";\(name)\"]}}}"
        #expect(connection.writeLine(message))
        guard case let .line(reply) = connection.readLine() else { throw ServiceError("No admitted mutation reply") }
        return reply
    }

    private func json(_ line: String) throws -> [String: Any] {
        try #require(try JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any])
    }

    @Test func agentToolTextMatchesSerdePrettyJSON() throws {
        let host = FakeAgentHost(library: .empty)
        let line = try #require(Agent.handle(host: host, serverVersion: "1", message:
            #"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_snippets","arguments":{}}}"#))
        let result = try #require(try json(line)["result"] as? [String: Any])
        let content = try #require(result["content"] as? [[String: Any]])
        #expect(content.first?["text"] as? String == """
        {
          "returned": 0,
          "snippets": [],
          "total": 0,
          "triggerPrefix": ";"
        }
        """)
    }

    @Test func asyncRepositorySocketRoundTrip() async throws {
        let dir = tempDir("async-agent")
        defer { try? FileManager.default.removeItem(at: dir) }
        let repository = try await LibraryRepository.open(.local(url: dir.appendingPathComponent("Library.store")))
        let host = FakeAgentHost(library: .empty)
        let server = AgentServer(dir: dir, version: "1", handle: { message in
            try? await repository.handleAgent(message: message, version: "1", host: host)
        }, allowed: { true })
        server.apply(on: true)
        let path = server.socketURL.path
        let reply = try await Task.detached {
            let connection = try #require(UnixConnection.connect(path))
            defer { connection.close() }
            connection.setReadTimeout(5)
            #expect(connection.writeLine(#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"create_snippet","arguments":{"name":"Socket","content":"saved","triggers":[";socket"]}}}"#))
            guard case let .line(line) = connection.readLine() else { throw ServiceError("No async reply") }
            return line
        }.value
        #expect(!reply.contains("isError\":true"))
        server.shutDown()
        await server.drain()
        let reopened = try await LibraryRepository.open(.local(url: dir.appendingPathComponent("Library.store")))
        #expect(reopened.snapshot.snippets.contains { $0.name == "Socket" && $0.content == "saved" })
    }

    @Test func socketRoundTrip() throws {
        let dir = tempDir("agent")
        defer { try? FileManager.default.removeItem(at: dir) }
        let host = FakeAgentHost(library: Library(snippets: [Snippet(name: "Sig", triggers: [";sig"], content: "Best")]))
        let allowed = Flag()
        let server = AgentServer(dir: dir, version: "9.9.9", makeHost: { host }, allowed: { allowed.value })
        server.apply(on: true)
        defer { server.shutDown() }
        #expect(server.isRunning)

        let path = server.socketURL.path
        let mode = try FileManager.default.attributesOfItem(atPath: path)[.posixPermissions] as? Int
        #expect(mode == 0o600, "owner-only")

        let conn = try #require(UnixConnection.connect(path))
        conn.setReadTimeout(5)
        #expect(conn.writeLine(#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#))
        guard case let .line(initLine) = conn.readLine() else { Issue.record("no initialize reply"); return }
        let initReply = try json(initLine)
        let result = try #require(initReply["result"] as? [String: Any])
        #expect(result["protocolVersion"] as? String == "2025-06-18")
        #expect((result["serverInfo"] as? [String: Any])?["version"] as? String == "9.9.9")

        // A notification gets no reply; the next request's reply comes straight after.
        #expect(conn.writeLine(#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#))
        #expect(conn.writeLine(#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#))
        guard case let .line(listLine) = conn.readLine() else { Issue.record("no tools/list reply"); return }
        let list = try json(listLine)
        #expect(list["id"] as? Int == 2)
        let tools = try #require((list["result"] as? [String: Any])?["tools"] as? [[String: Any]])
        let names = tools.compactMap { $0["name"] as? String }
        #expect(names.contains("list_snippets"))
        #expect(names.contains("create_snippet"))
        #expect(!names.contains("list_clipboard"), "clipboard isn't shared with this host")

        // Turning access off ends open sessions.
        allowed.value = false
        #expect(conn.writeLine(#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#))
        #expect(conn.readLine() == .end)

        server.shutDown()
        #expect(!server.isRunning)
        #expect(!FileManager.default.fileExists(atPath: path), "socket removed")
    }

    @Test func refusesASecondServerAndReplacesAStaleSocket() throws {
        let dir = tempDir("agent2")
        defer { try? FileManager.default.removeItem(at: dir) }
        let host = FakeAgentHost(library: .empty)
        let first = AgentServer(dir: dir, version: "1", makeHost: { host }, allowed: { true })
        first.apply(on: true)
        let second = AgentServer(dir: dir, version: "1", makeHost: { host }, allowed: { true })
        second.apply(on: true)
        #expect(!second.isRunning, "another Kudzu is already answering agents")
        first.shutDown()

        // Left over from a crash: a socket file nobody listens on.
        let stale = try UnixSocket.listen(dir.appendingPathComponent(AgentServer.socketName).path)
        close(stale)
        second.apply(on: true)
        #expect(second.isRunning)
        second.shutDown()
    }

    /// Port of `relays_through_a_socket_and_falls_back_offline`.
    @Test func bridgeRelaysAndFallsBackOffline() throws {
        let dir = tempDir("bridge")
        defer { try? FileManager.default.removeItem(at: dir) }
        let path = dir.appendingPathComponent(AgentServer.socketName).path
        let relay = MCPBridge.Relay(path: path)
        let ping = #"{"jsonrpc":"2.0","id":1,"method":"ping"}"#
        #expect(relay.send(ping) == .undelivered, "nothing listening")

        let server = AgentServer(dir: dir, version: "9", makeHost: { Agent.Offline() }, allowed: { true })
        server.apply(on: true)
        defer { server.shutDown() }
        #expect(relay.send(#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#) == .delivered(nil))
        guard case let .delivered(reply?) = relay.send(ping) else { Issue.record("no ping reply"); return }
        let object = try json(reply)
        #expect(object["id"] as? Int == 1)
        #expect((object["result"] as? [String: Any])?.isEmpty == true)

        // Kudzu restarted: the old connection fails once, then the relay reconnects.
        server.shutDown()
        #expect(relay.send(ping) == .undelivered)
        server.apply(on: true)
        guard case .delivered(_?) = relay.send(ping) else { Issue.record("no reconnect"); return }
    }

    @Test func bridgeSocketFollowsHOMEAndVariant() {
        #expect(MCPBridge.socketPath(dev: true, environment: ["HOME": "/Users/a"]) == "/Users/a/Library/Application Support/sh.saeed.kudzu.dev/agent.sock")
        #expect(MCPBridge.socketPath(dev: false, environment: ["HOME": "/Users/a"]) == "/Users/a/Library/Application Support/sh.saeed.kudzu/agent.sock")
    }

    @Test func quotesPathsForTheShell() {
        #expect(AgentSetupInfo.shellQuote("/Applications/Kudzu.app/Contents/MacOS/Kudzu") == "/Applications/Kudzu.app/Contents/MacOS/Kudzu")
        #expect(AgentSetupInfo.shellQuote("/Users/a b/Kudzu Dev.app/x") == "'/Users/a b/Kudzu Dev.app/x'")
        #expect(AgentSetupInfo.shellQuote("it's") == #"'it'\''s'"#)
    }

    @Test func setupForReleaseAndDev() throws {
        let dir = tempDir("setup")
        defer { try? FileManager.default.removeItem(at: dir) }
        let skill = dir.appendingPathComponent("skills/kudzu/SKILL.md")
        let dev = AgentSetupInfo.make(
            identity: AppIdentity(bundleID: AppIdentity.devID, version: "0.6.0", productName: "Kudzu Dev"),
            running: true, executable: "/Applications/Kudzu Dev.app/Contents/MacOS/Kudzu Dev", skillURL: skill, bundledSkillDir: nil
        )
        #expect(dev.name == "kudzu-dev")
        #expect(dev.args == ["--mcp", "--dev"])
        #expect(dev.claudeCommand == "claude mcp add --scope user kudzu-dev -- '/Applications/Kudzu Dev.app/Contents/MacOS/Kudzu Dev' --mcp --dev")
        #expect(dev.skill == .missing)
        #expect(dev.configJson == """
        {
          "mcpServers": {
            "kudzu-dev": {
              "args": [
                "--mcp",
                "--dev"
              ],
              "command": "/Applications/Kudzu Dev.app/Contents/MacOS/Kudzu Dev"
            }
          }
        }
        """)
        let parsed = try #require(try JSONSerialization.jsonObject(with: Data(dev.configJson.utf8)) as? [String: Any])
        #expect(parsed["mcpServers"] != nil)

        #expect(try AgentSetupInfo.installSkill(at: skill) == skill.path)
        let release = AgentSetupInfo.make(
            identity: AppIdentity(bundleID: AppIdentity.productionID, version: "0.6.0", productName: "Kudzu"),
            running: false, executable: "/Applications/Kudzu.app/Contents/MacOS/Kudzu", skillURL: skill, bundledSkillDir: nil
        )
        #expect(release.name == "kudzu")
        #expect(release.claudeCommand == "claude mcp add --scope user kudzu -- /Applications/Kudzu.app/Contents/MacOS/Kudzu --mcp")
        #expect(release.skill == .current)
        write("old", to: skill)
        let outdated = AgentSetupInfo.make(
            identity: AppIdentity(bundleID: AppIdentity.productionID, version: "0.6.0", productName: "Kudzu"),
            running: false, executable: "/k", skillURL: skill, bundledSkillDir: nil
        )
        #expect(outdated.skill == .outdated)
    }
}

private actor SocketRequestGate {
    private var continuation: CheckedContinuation<Void, Never>?
    private var isOpen = false
    var isWaiting: Bool { continuation != nil }
    func wait() async {
        guard !isOpen else { return }
        await withCheckedContinuation { continuation = $0 }
    }
    func open() {
        isOpen = true
        continuation?.resume()
        continuation = nil
    }
}

@Suite struct UnixConnectionTests {
    /// A connected pair: what's written to `.1` is read from `.0`.
    private func pair(maxLineBytes: Int = UnixConnection.maxLineBytes) -> (UnixConnection, Int32) {
        var fds: [Int32] = [0, 0]
        socketpair(AF_UNIX, SOCK_STREAM, 0, &fds)
        return (UnixConnection(fd: fds[0], maxLineBytes: maxLineBytes), fds[1])
    }

    private func send(_ text: String, to fd: Int32) {
        _ = Array(text.utf8).withUnsafeBytes { Darwin.write(fd, $0.baseAddress, $0.count) }
    }

    @Test func splitsLinesAcrossAndWithinReads() {
        let (conn, peer) = pair()
        send("one\r\ntw", to: peer)
        #expect(conn.readLine() == .line("one"))
        send("o\nthree\n", to: peer)
        #expect(conn.readLine() == .line("two"))
        #expect(conn.readLine() == .line("three"))
        send("tail", to: peer)
        close(peer)
        #expect(conn.readLine() == .line("tail"))
        #expect(conn.readLine() == .end)
    }

    @Test func disconnectsAPeerThatNeverSendsANewline() {
        let (conn, peer) = pair(maxLineBytes: 1024)
        send(String(repeating: "x", count: 4096), to: peer)
        #expect(conn.readLine() == .end)
        close(peer)
    }

    @Test func aLineAtTheLimitIsStillRead() {
        let (conn, peer) = pair(maxLineBytes: 1024)
        let line = String(repeating: "y", count: 1024)
        send(line + "\n", to: peer)
        #expect(conn.readLine() == .line(line))
        close(peer)
    }

    @Test func aNewlineDoesNotExcuseALinePastTheCap() {
        let (conn, peer) = pair(maxLineBytes: 1024)
        send(String(repeating: "z", count: 1025) + "\n", to: peer)
        #expect(conn.readLine() == .end)
        close(peer)
    }
}
