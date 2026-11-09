import Darwin
import Foundation
import KudzuCore

/// `Kudzu --mcp [--dev]`: relays an agent's stdio to the running app, one JSON-RPC message per
/// line (`agent::bridge`). While the app can't be reached it still answers, so the agent sees the
/// tools and a message saying what to turn on.
public enum MCPBridge {
    /// How long the bridge waits for the app to answer one request.
    public static let replyTimeoutSeconds = 30

    /// The socket the running app listens on.
    /// Like `agent::bridge`, it starts from `$HOME` (the account's home folder without it).
    public static func socketPath(dev: Bool, environment: [String: String] = ProcessInfo.processInfo.environment) -> String {
        let home = environment["HOME"].map { URL(fileURLWithPath: $0, isDirectory: true) } ?? FileManager.default.homeDirectoryForCurrentUser
        return home.appendingPathComponent("Library/Application Support", isDirectory: true)
            .appendingPathComponent(dev ? AppIdentity.devID : AppIdentity.productionID, isDirectory: true)
            .appendingPathComponent(AgentServer.socketName).path
    }

    /// Runs the bridge until stdin closes. Returns the exit code.
    public static func run(dev: Bool, version: String = AppIdentity.current.version) -> Int32 {
        let relay = Relay(path: socketPath(dev: dev))
        let out = FileHandle.standardOutput
        while let line = Swift.readLine(strippingNewline: true) {
            if line.trimmingCharacters(in: .whitespaces).isEmpty { continue }
            let reply: String?
            switch relay.send(line) {
            case let .delivered(answer): reply = answer
            case .undelivered: reply = Agent.handle(host: Agent.Offline(), serverVersion: version, message: line)
            }
            if let reply {
                do {
                    try out.write(contentsOf: Data((reply + "\n").utf8))
                } catch {
                    break
                }
            }
        }
        return 0
    }

    /// One connection to the app, reopened when Kudzu restarts.
    final class Relay {
        enum Outcome: Equatable {
            /// Delivered; the reply, or `nil` when none is due (a notification).
            case delivered(String?)
            /// The app couldn't be reached (or didn't answer in time).
            case undelivered
        }

        let path: String
        private var conn: UnixConnection?

        init(path: String) { self.path = path }

        func send(_ line: String) -> Outcome {
            // A connection from before Kudzu restarted fails once; reconnect and try again.
            for _ in 0 ..< 2 {
                if conn == nil {
                    guard let c = UnixConnection.connect(path) else { return .undelivered }
                    c.setReadTimeout(MCPBridge.replyTimeoutSeconds)
                    conn = c
                }
                guard let c = conn else { return .undelivered }
                if c.writeLine(line) {
                    if !Agent.expectsReply(line) { return .delivered(nil) }
                    switch c.readLine() {
                    case let .line(reply): return .delivered(reply)
                    case .timedOut:
                        // The request may have run; don't send it twice.
                        conn = nil
                        return .undelivered
                    case .end: break
                    }
                }
                conn = nil
            }
            return .undelivered
        }
    }
}
