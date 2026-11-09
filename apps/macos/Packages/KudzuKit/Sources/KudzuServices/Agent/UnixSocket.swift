import Darwin
import Foundation

/// A connected Unix-domain stream socket, read and written as newline-delimited lines.
/// Owns its file descriptor.
final class UnixConnection: @unchecked Sendable {
    // @unchecked: one connection is used by one thread at a time (a client thread, or the
    // bridge's main loop); `pending` is only touched by the reader.
    let fd: Int32
    /// Longest line accepted; a peer that sends more without a newline is disconnected.
    static let maxLineBytes = 16 << 20
    private let maxLineBytes: Int
    /// Received bytes; `pending[head...]` is unread and `pending[head..<head + scanned]` holds
    /// no newline, so each byte is searched once.
    private var pending: [UInt8] = []
    private var head = 0
    private var scanned = 0
    private var buffer = [UInt8](repeating: 0, count: 64 * 1024)
    private var closed = false

    init(fd: Int32, maxLineBytes: Int = UnixConnection.maxLineBytes) {
        self.fd = fd
        self.maxLineBytes = maxLineBytes
        var on: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &on, socklen_t(MemoryLayout<Int32>.size))
    }

    deinit { close() }

    func close() {
        guard !closed else { return }
        closed = true
        Darwin.close(fd)
    }

    /// Connects to the socket at `path`; `nil` when nothing listens there.
    static func connect(_ path: String) -> UnixConnection? {
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { return nil }
        var addr = UnixSocket.address(path)
        let ok = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) == 0
            }
        }
        guard ok else {
            Darwin.close(fd)
            return nil
        }
        return UnixConnection(fd: fd)
    }

    /// Fails reads that wait longer than `seconds` (`EAGAIN`).
    func setReadTimeout(_ seconds: Int) {
        var tv = timeval(tv_sec: seconds, tv_usec: 0)
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))
    }

    /// Writes `line` plus a newline. False when the peer is gone.
    func writeLine(_ line: String) -> Bool {
        let data = Data((line + "\n").utf8)
        return data.withUnsafeBytes { raw -> Bool in
            guard let base = raw.baseAddress else { return true }
            var offset = 0
            while offset < raw.count {
                let n = Darwin.write(fd, base.advanced(by: offset), raw.count - offset)
                if n < 0 {
                    if errno == EINTR { continue }
                    return false
                }
                offset += n
            }
            return true
        }
    }

    enum ReadResult: Equatable {
        case line(String)
        case end
        case timedOut
    }

    /// The next line without its newline. `.end` when the peer closes, fails, or sends a line
    /// longer than `maxLineBytes`.
    func readLine() -> ReadResult {
        while true {
            if let nl = pending[(head + scanned)...].firstIndex(of: UInt8(ascii: "\n")) {
                if nl - head > maxLineBytes {
                    consume(upTo: pending.count)
                    return .end
                }
                var text = String(decoding: pending[head ..< nl], as: UTF8.self)
                consume(upTo: nl + 1)
                if text.hasSuffix("\r") { text.removeLast() }
                return .line(text)
            }
            scanned = pending.count - head
            guard scanned <= maxLineBytes else {
                consume(upTo: pending.count)
                return .end
            }
            let n = buffer.withUnsafeMutableBytes { Darwin.read(fd, $0.baseAddress, $0.count) }
            if n > 0 {
                pending.append(contentsOf: buffer[0 ..< n])
            } else if n == 0 {
                if head == pending.count { return .end }
                let text = String(decoding: pending[head...], as: UTF8.self)
                consume(upTo: pending.count)
                return .line(text)
            } else if errno == EINTR {
                continue
            } else if errno == EAGAIN || errno == EWOULDBLOCK {
                return .timedOut
            } else {
                return .end
            }
        }
    }

    /// Marks `pending[..<end]` read, compacting once the read part dominates the buffer.
    private func consume(upTo end: Int) {
        head = end
        scanned = 0
        if head == pending.count {
            pending.removeAll(keepingCapacity: pending.count <= 1 << 20)
            head = 0
        } else if head >= 64 * 1024, head * 2 >= pending.count {
            pending.removeFirst(head)
            head = 0
        }
    }
}

enum UnixSocket {
    static func address(_ path: String) -> sockaddr_un {
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8.prefix(MemoryLayout.size(ofValue: addr.sun_path) - 1))
        withUnsafeMutableBytes(of: &addr.sun_path) { raw in
            raw.copyBytes(from: bytes)
            raw[bytes.count] = 0
        }
        return addr
    }

    /// Binds and listens on `path`, owner-only (0600).
    static func listen(_ path: String) throws -> Int32 {
        guard path.utf8.count < MemoryLayout.size(ofValue: sockaddr_un().sun_path) else {
            throw ServiceError("The socket path is too long: \(path)")
        }
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw error("socket") }
        var addr = address(path)
        // Bind, then restrict (`agent.rs`). No process-wide `umask`: it would also apply to
        // folders other threads create meanwhile. The socket lives in the data folder inside
        // the owner-only ~/Library, so nobody else can reach it before the `chmod`.
        let bound = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bound == 0 else {
            let e = error("bind")
            close(fd)
            throw e
        }
        guard chmod(path, 0o600) == 0 else {
            let e = error("chmod")
            close(fd)
            unlink(path)
            throw e
        }
        guard Darwin.listen(fd, 16) == 0 else {
            let e = error("listen")
            close(fd)
            unlink(path)
            throw e
        }
        return fd
    }

    static func error(_ op: String) -> ServiceError {
        ServiceError("\(op): \(String(cString: strerror(errno)))")
    }
}
