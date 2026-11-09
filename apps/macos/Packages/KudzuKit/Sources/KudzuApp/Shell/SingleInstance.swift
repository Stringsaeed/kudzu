import AppKit
import Darwin
import KudzuServices

/// One running copy per bundle id, across both Kudzu builds that share it.
///
/// - A second native launch asks the first to show its main window through a distributed
///   notification, then exits.
/// - The Tauri app (tauri-plugin-single-instance) looks for a running copy by connecting to the
///   Unix socket `/tmp/<identifier, . and - as _>_si.sock` and exits as soon as that works. This
///   copy listens on the same socket, so a stray Tauri launch hands off to it instead of loading
///   the same `library.json` and installing a second event tap.
/// - A Tauri copy that is already running is never run alongside: the user can quit it (this
///   launch then continues) or keep it (this launch shows it and exits).
@MainActor
final class SingleInstance {
    let bundleID: String
    let socketPath: String
    var onShowMain: (() -> Void)? {
        didSet {
            if showMainPending, let onShowMain {
                showMainPending = false
                onShowMain()
            }
        }
    }
    private var observer: NSObjectProtocol?
    private var terminateObserver: NSObjectProtocol?
    private var socket: InstanceSocket?
    private var showMainPending = false

    init(bundleID: String) {
        self.bundleID = bundleID
        socketPath = Self.socketPath(for: bundleID)
    }

    isolated deinit {
        if let observer { DistributedNotificationCenter.default().removeObserver(observer) }
        if let terminateObserver { NotificationCenter.default.removeObserver(terminateObserver) }
        socket?.close()
    }

    /// tauri-plugin-single-instance's `socket_path` (without its optional `semver` feature).
    nonisolated static func socketPath(for bundleID: String) -> String {
        let identifier = String(bundleID.map { $0 == "." || $0 == "-" ? "_" : $0 })
        return "/tmp/\(identifier)_si.sock"
    }

    /// The Tauri build's executable (`kudzu-desktop`); the native one is `Kudzu`.
    nonisolated static func isTauriBuild(executable: URL?) -> Bool {
        executable?.lastPathComponent == "kudzu-desktop"
    }

    private var name: Notification.Name { Notification.Name("\(bundleID).show-main") }

    /// A second launch can arrive while the backend is still loading. Keep that request until
    /// AppDelegate attaches the shell, including when the first launch was hidden.
    func requestShowMain() {
        if let onShowMain { onShowMain() }
        else { showMainPending = true }
    }

    /// True when another copy is running (it was told to show its window) and this launch
    /// should exit. Otherwise claims the instance and starts listening for later launches.
    func handOffToRunningCopy() -> Bool {
        let me = ProcessInfo.processInfo.processIdentifier
        let others = NSRunningApplication.runningApplications(withBundleIdentifier: bundleID).filter { $0.processIdentifier != me }
        if let tauri = others.first(where: { Self.isTauriBuild(executable: $0.executableURL) }) {
            if keepTauriCopy(tauri) { return true }
        } else if let peer = InstanceSocket.notify(socketPath) {
            // Something holds the socket that Launch Services doesn't list (yet): still starting.
            let app = NSRunningApplication(processIdentifier: peer)
            if let app, Self.isTauriBuild(executable: app.executableURL) {
                if keepTauriCopy(app) { return true }
            } else {
                app?.activate()
                return true
            }
        }
        return !claim()
    }

    private func claim() -> Bool {
        guard let socket = InstanceSocket.listen(socketPath, onConnect: { [weak self] in
            Task { @MainActor in self?.requestShowMain() }
        }) else {
            _ = InstanceSocket.notify(socketPath)
            DistributedNotificationCenter.default().postNotificationName(name, object: nil, userInfo: nil, deliverImmediately: true)
            Logging.logger(category: "app").warning("single instance: ownership was not acquired; refusing a second data writer")
            return false
        }
        self.socket = socket
        observer = DistributedNotificationCenter.default().addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.requestShowMain() }
        }
        // The socket goes with this process, as the plugin's does.
        terminateObserver = NotificationCenter.default.addObserver(forName: NSApplication.willTerminateNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.socket?.close() }
        }
        // Now that this copy owns the instance, replace the Tauri app's login agent (which
        // would start the old executable at every login) with the native login item.
        if Bundle.main.bundleURL.pathExtension == "app" {
            let productName = Bundle.main.object(forInfoDictionaryKey: "CFBundleName") as? String ?? "Kudzu"
            LoginItem.migrateLegacyLaunchAgent(productName: productName, bundleID: bundleID)
        }
        return true
    }

    /// An earlier (Tauri) Kudzu is running. Two writers on one `library.json` overwrite each
    /// other's edits, so ask: quit it and continue (false), or keep it and exit (true).
    private func keepTauriCopy(_ app: NSRunningApplication) -> Bool {
        NSApp.activate()
        let alert = NSAlert()
        alert.messageText = "An earlier version of Kudzu is open"
        alert.informativeText = "Only one copy of Kudzu can run at a time, because both would save your snippets. Quit the earlier version and open this one?"
        alert.addButton(withTitle: "Quit Earlier Version")
        alert.addButton(withTitle: "Keep It Open")
        if alert.runModal() == .alertFirstButtonReturn {
            if Self.quit(app) { return false }
            let failed = NSAlert()
            failed.messageText = "Kudzu couldn't quit the earlier version"
            failed.informativeText = "Quit it from its menu bar icon, then open Kudzu again."
            failed.runModal()
            return true
        }
        _ = InstanceSocket.notify(socketPath) // its single-instance callback shows its window
        app.activate()
        return true
    }

    /// Asks `app` to quit, then forces it after 10 s. Its saves are synchronous and atomic, so
    /// a forced quit can't leave a half-written library. True once the process is gone.
    private static func quit(_ app: NSRunningApplication) -> Bool {
        let pid = app.processIdentifier
        func gone(within seconds: TimeInterval) -> Bool {
            let deadline = Date().addingTimeInterval(seconds)
            while Date() < deadline {
                if kill(pid, 0) != 0, errno == ESRCH { return true }
                RunLoop.current.run(mode: .default, before: Date().addingTimeInterval(0.1))
            }
            return kill(pid, 0) != 0 && errno == ESRCH
        }
        app.terminate()
        if gone(within: 10) { return true }
        app.forceTerminate()
        return gone(within: 3)
    }
}

/// The listening end of the single-instance socket, in the plugin's wire format: a connecting
/// copy writes `cwd \0\0 args joined by \0` and closes.
final class InstanceSocket: @unchecked Sendable {
    // @unchecked: `fd`/`inode` are set once; `closed` is only touched while holding `lock`.
    private let path: String
    private let fd: Int32
    private let claimFD: Int32
    private let wakeRead: Int32
    private let wakeWrite: Int32
    private let inode: ino_t
    private let lock = NSLock()
    private var closed = false
    private let stopped = DispatchGroup()

    private init(path: String, fd: Int32, claimFD: Int32, wakeRead: Int32, wakeWrite: Int32, inode: ino_t) {
        self.path = path
        self.fd = fd
        self.claimFD = claimFD
        self.wakeRead = wakeRead
        self.wakeWrite = wakeWrite
        self.inode = inode
        stopped.enter()
    }

    /// Connects to a listening copy and sends this launch's arguments. Returns the listener's
    /// pid, or `nil` when nothing listens there.
    static func notify(_ path: String) -> pid_t? {
        let fd = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { return nil }
        defer { Darwin.close(fd) }
        var on: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &on, socklen_t(MemoryLayout<Int32>.size))
        guard connect(fd, path) == 0 else { return nil }
        var peer: pid_t = 0
        var size = socklen_t(MemoryLayout<pid_t>.size)
        getsockopt(fd, SOL_LOCAL, LOCAL_PEERPID, &peer, &size)
        let message = Data((FileManager.default.currentDirectoryPath + "\0\0" + CommandLine.arguments.joined(separator: "\0")).utf8)
        message.withUnsafeBytes { raw in
            guard let base = raw.baseAddress else { return }
            var offset = 0
            while offset < raw.count {
                let n = Darwin.write(fd, base.advanced(by: offset), raw.count - offset)
                if n < 0, errno == EINTR { continue }
                if n <= 0 { return }
                offset += n
            }
        }
        return peer
    }

    /// Native launches hold one exclusive lifetime claim before examining/removing a stale
    /// socket. The lock file is never unlinked: every contender must lock the same inode.
    /// A failed claim refuses startup; an earlier socket check is not ownership evidence.
    static func listen(_ path: String, onConnect: @escaping @Sendable () -> Void) -> InstanceSocket? {
        let log = Logging.logger(category: "app")
        let claimFD = Darwin.open(path + ".lock", O_RDWR | O_CREAT | O_CLOEXEC | O_NOFOLLOW, 0o600)
        guard claimFD >= 0 else { return nil }
        var claimed = false
        defer { if !claimed { Darwin.close(claimFD) } }
        var lockInfo = stat()
        guard fstat(claimFD, &lockInfo) == 0, lockInfo.st_mode & S_IFMT == S_IFREG,
              lockInfo.st_uid == getuid(), flock(claimFD, LOCK_EX | LOCK_NB) == 0 else { return nil }
        var st = stat()
        if lstat(path, &st) == 0 {
            guard st.st_mode & S_IFMT == S_IFSOCK, st.st_uid == getuid() else {
                log.warning("single instance: \(path) is not this user's socket; refusing startup")
                return nil
            }
            guard notify(path) == nil else { return nil }
            var current = stat()
            guard lstat(path, &current) == 0, current.st_ino == st.st_ino,
                  current.st_uid == getuid(), unlink(path) == 0 else { return nil }
        }
        let fd = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { return nil }
        _ = fcntl(fd, F_SETFD, FD_CLOEXEC)
        guard bind(fd, path) == 0, chmod(path, 0o600) == 0, Darwin.listen(fd, 8) == 0, lstat(path, &st) == 0 else {
            log.warning("single instance: couldn't listen on \(path): \(String(cString: strerror(errno)))")
            Darwin.close(fd)
            return nil
        }
        var wake: [Int32] = [-1, -1]
        guard pipe(&wake) == 0 else { Darwin.close(fd); return nil }
        for descriptor in wake { _ = fcntl(descriptor, F_SETFD, FD_CLOEXEC) }
        guard fcntl(fd, F_SETFL, O_NONBLOCK) == 0 else {
            Darwin.close(fd)
            for descriptor in wake { Darwin.close(descriptor) }
            return nil
        }
        let socket = InstanceSocket(path: path, fd: fd, claimFD: claimFD, wakeRead: wake[0], wakeWrite: wake[1], inode: st.st_ino)
        claimed = true
        let thread = Thread {
            defer { socket.finish() }
            while !socket.isClosed {
                var watched = [pollfd(fd: fd, events: Int16(POLLIN), revents: 0),
                               pollfd(fd: socket.wakeRead, events: Int16(POLLIN), revents: 0)]
                let ready = poll(&watched, nfds_t(watched.count), -1)
                if ready < 0, errno == EINTR { continue }
                guard ready > 0, watched[1].revents == 0, !socket.isClosed,
                      watched[0].revents & Int16(POLLIN) != 0 else { return }
                let client = accept(fd, nil, nil)
                if client < 0 {
                    if errno == EINTR || errno == ECONNABORTED || errno == EAGAIN { continue }
                    return // closed
                }
                _ = fcntl(client, F_SETFD, FD_CLOEXEC)
                // Read the launch's arguments to the end (bounded), as the plugin does, then close.
                var tv = timeval(tv_sec: 2, tv_usec: 0)
                setsockopt(client, SOL_SOCKET, SO_RCVTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))
                var buffer = [UInt8](repeating: 0, count: 4096)
                var total = 0
                while total < 64 * 1024 {
                    let n = buffer.withUnsafeMutableBytes { Darwin.read(client, $0.baseAddress, $0.count) }
                    if n < 0, errno == EINTR { continue }
                    if n <= 0 { break }
                    total += n
                }
                Darwin.close(client)
                if !socket.isClosed { onConnect() }
            }
        }
        thread.name = "kudzu-single-instance"
        thread.start()
        return socket
    }

    private var isClosed: Bool { lock.withLock { closed } }

    /// Requests shutdown without waiting on a callback or the main actor. Only the accept
    /// worker closes descriptors, so they cannot be reused while it can still call accept.
    func close() {
        lock.withLock {
            guard !closed else { return }
            closed = true
            var byte: UInt8 = 1
            while true {
                let result = withUnsafePointer(to: &byte) { Darwin.write(wakeWrite, $0, 1) }
                if result < 0, errno == EINTR { continue }
                break
            }
        }
    }

    /// Lets lifecycle tests await completion without blocking the main actor.
    func waitForShutdown() async {
        await withCheckedContinuation { continuation in
            stopped.notify(queue: .global()) { continuation.resume() }
        }
    }

    private func finish() {
        lock.withLock {
            closed = true
            var st = stat()
            if lstat(path, &st) == 0, st.st_ino == inode { unlink(path) }
            Darwin.close(fd)
            Darwin.close(claimFD)
            Darwin.close(wakeRead)
            Darwin.close(wakeWrite)
        }
        stopped.leave()
    }

    private static func address(_ path: String) -> sockaddr_un? {
        var addr = sockaddr_un()
        let capacity = MemoryLayout.size(ofValue: addr.sun_path)
        let bytes = Array(path.utf8)
        guard bytes.count < capacity else { return nil }
        addr.sun_family = sa_family_t(AF_UNIX)
        withUnsafeMutableBytes(of: &addr.sun_path) { raw in
            raw.copyBytes(from: bytes)
            raw[bytes.count] = 0
        }
        return addr
    }

    private static func connect(_ fd: Int32, _ path: String) -> Int32 {
        guard var addr = address(path) else { return -1 }
        return withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
        }
    }

    private static func bind(_ fd: Int32, _ path: String) -> Int32 {
        guard var addr = address(path) else { return -1 }
        return withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
        }
    }
}
