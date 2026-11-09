import Foundation
import os

/// App logging: `os.Logger` (Console.app, subsystem = bundle id) plus a rotating file at
/// `~/Library/Logs/<bundle id>/Kudzu.log` (2 MB, three files kept), like tauri-plugin-log in
/// `lib.rs`. Debug builds log at debug level; release builds from info up.
///
/// Users send the file when sync or updates misbehave (RELEASING.md), so everything that
/// matters goes through here; app code never uses `print`.
public enum Logging {
    /// Points the file log at this identity's log folder. Call once at launch; until then,
    /// messages go to the unified log only.
    public static func configure(identity: AppIdentity) {
        FileLog.shared.open(directory: identity.logDirectory)
        subsystemBox.set(identity.bundleID)
    }

    /// A logger for one area of the app ("store", "clipboard", "updater", …).
    public static func logger(category: String) -> Log {
        Log(category: category, subsystem: subsystemBox.get())
    }

    /// Whether debug messages are recorded.
    public static let debugEnabled: Bool = {
        #if DEBUG
            true
        #else
            false
        #endif
    }()

    private static let subsystemBox = SubsystemBox()

    /// The subsystem name, set once at launch.
    private final class SubsystemBox: @unchecked Sendable {
        // @unchecked: `value` is only touched while holding `lock`.
        private let lock = NSLock()
        private var value = Bundle.main.bundleIdentifier ?? AppIdentity.devID
        func get() -> String { lock.withLock { value } }
        func set(_ v: String) { lock.withLock { value = v } }
    }
}

/// One logging category. Messages are public in the unified log (they never carry snippet
/// content or clipboard text; callers keep it that way).
public struct Log: Sendable {
    public let category: String
    private let logger: Logger

    init(category: String, subsystem: String) {
        self.category = category
        logger = Logger(subsystem: subsystem, category: category)
    }

    public func debug(_ message: @autoclosure () -> String) {
        guard Logging.debugEnabled else { return }
        let m = message()
        logger.debug("\(m, privacy: .public)")
        FileLog.shared.write(level: "DEBUG", category: category, message: m)
    }

    public func info(_ message: @autoclosure () -> String) {
        let m = message()
        logger.info("\(m, privacy: .public)")
        FileLog.shared.write(level: "INFO", category: category, message: m)
    }

    public func warning(_ message: @autoclosure () -> String) {
        let m = message()
        logger.warning("\(m, privacy: .public)")
        FileLog.shared.write(level: "WARN", category: category, message: m)
    }

    public func error(_ message: @autoclosure () -> String) {
        let m = message()
        logger.error("\(m, privacy: .public)")
        FileLog.shared.write(level: "ERROR", category: category, message: m)
    }
}
