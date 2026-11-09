import Darwin
import Foundation
import KudzuCore

/// AI agent access over the Model Context Protocol. Port of `agent.rs`.
///
/// ```text
/// agent (Claude Code, …) ──stdio──▶ Kudzu --mcp ──agent.sock──▶ running Kudzu ──▶ library, clipboard
/// ```
///
/// The protocol, tools and secret redaction live in `KudzuCore.Agent`; this type moves lines
/// between them. The socket exists only while **Allow AI agents** is on. It's owner-only (0600)
/// inside the app's data folder, so only this user's processes can reach it, and they could read
/// the library files anyway; what the socket adds is redaction and safe writes.
public final class AgentServer: @unchecked Sendable {
    var testingAcceptedRequests: Int { requests.testingAcceptedCount }
    // @unchecked: `running` is only touched while holding `lock`; each `Running` is shared
    // with its accept thread, which only reads its immutable fields and the atomic flag.

    /// The socket's file name in the data folder.
    public static let socketName = "agent.sock"

    final class Running: @unchecked Sendable {
        // @unchecked: `stopped` is guarded by `flagLock`; the rest is immutable.
        let path: String
        let listener: Int32
        private let flagLock = NSLock()
        private var stopped = false

        init(path: String, listener: Int32) {
            self.path = path
            self.listener = listener
        }

        var isStopped: Bool { flagLock.withLock { stopped } }

        /// Stops accepting; the accept thread closes the listener when it sees the flag.
        func shutDown() {
            flagLock.withLock { stopped = true }
            // Wake the accept loop so it sees the flag.
            _ = UnixConnection.connect(path)
            unlink(path)
        }
    }

    public let socketURL: URL
    private let version: String
    private let makeHost: @Sendable () -> any Agent.Host
    private let handle: (@Sendable (String) async -> String?)?
    private let requests = AsyncWorkQueue()
    private let allowed: @Sendable () -> Bool
    private let lock = NSLock()
    private var running: Running?
    private let log = Logging.logger(category: "agent")

    /// - Parameters:
    ///   - dir: the data folder (`agent.sock` goes inside).
    ///   - makeHost: the library/clipboard host for one client session.
    ///   - allowed: whether **Allow AI agents** is still on (checked per message, so turning it
    ///     off ends open sessions too).
    public init(dir: URL, version: String, makeHost: @escaping @Sendable () -> any Agent.Host, allowed: @escaping @Sendable () -> Bool) {
        socketURL = dir.appendingPathComponent(Self.socketName)
        self.version = version
        self.handle = nil
        self.makeHost = makeHost
        self.allowed = allowed
    }

    public init(dir: URL, version: String, handle: @escaping @Sendable (String) async -> String?, allowed: @escaping @Sendable () -> Bool) {
        socketURL = dir.appendingPathComponent(Self.socketName)
        self.version = version; self.handle = handle; self.allowed = allowed
        makeHost = { Agent.Offline() }
    }
    public func drain() async { await requests.drain() }

    public var isRunning: Bool { lock.withLock { running != nil } }

    /// Starts or stops the socket to match **Allow AI agents**.
    public func apply(on: Bool) {
        lock.lock()
        defer { lock.unlock() }
        if on, running == nil {
            do {
                running = try listen()
            } catch {
                log.warning("agent access: couldn't open the socket: \(error.localizedDescription)")
            }
        } else if !on, let r = running {
            running = nil
            r.shutDown()
            log.info("agent access off")
        }
    }

    /// Removes the socket on quit.
    public func shutDown() { apply(on: false) }

    private func listen() throws -> Running {
        let path = socketURL.path
        if FileManager.default.fileExists(atPath: path) {
            if UnixConnection.connect(path) != nil {
                throw ServiceError("another Kudzu is already answering agents")
            }
            // Left over from a crash.
            unlink(path)
        }
        let fd = try UnixSocket.listen(path)
        let r = Running(path: path, listener: fd)
        let thread = Thread { [makeHost, allowed, version, log, handle, requests] in
            while true {
                let client = accept(fd, nil, nil)
                if r.isStopped {
                    if client >= 0 { close(client) }
                    break
                }
                if client < 0 {
                    if errno == EINTR || errno == ECONNABORTED { continue }
                    log.warning("agent access: \(String(cString: strerror(errno)))")
                    break
                }
                let conn = UnixConnection(fd: client)
                let worker = Thread {
                    if let handle {
                        while case let .line(line) = conn.readLine() {
                            if r.isStopped || !allowed() { break }
                            if line.trimmingCharacters(in: .whitespaces).isEmpty { continue }
                            let completed = DispatchSemaphore(value: 0)
                            let accepted = requests.enqueue {
                                // Shutdown stops admission, but already queued requests finish.
                                // Explicit access revocation still cancels their authorization.
                                if allowed(), let reply = await handle(line) { _ = conn.writeLine(reply) }
                                completed.signal()
                            }
                            if !accepted { break }
                            completed.wait()
                        }
                        conn.close()
                    } else {
                        Self.serve(conn, host: makeHost(), running: r, allowed: allowed, version: version)
                    }
                }
                worker.name = "kudzu-agent-client"
                worker.start()
            }
            close(fd)
        }
        thread.name = "kudzu-agent"
        thread.start()
        log.info("agent access on")
        return r
    }

    private static func serve(_ conn: UnixConnection, host: any Agent.Host, running: Running, allowed: () -> Bool, version: String) {
        while case let .line(line) = conn.readLine() {
            // Turning access off ends open sessions too.
            if running.isStopped || !allowed() { break }
            if line.trimmingCharacters(in: .whitespaces).isEmpty { continue }
            if let reply = Agent.handle(host: host, serverVersion: version, message: line), !conn.writeLine(reply) { break }
        }
        conn.close()
    }
}
