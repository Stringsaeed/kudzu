import Foundation
import os

/// A value guarded by a lock: the Swift counterpart of the Rust engine's `Mutex<T>`.
///
/// `@unchecked Sendable`: `value` is only ever read or written inside `withLock`, which holds
/// `lock` for the whole access, so sharing the box between the tap, worker and poll threads is
/// race-free. The lock is not reentrant (like `std::sync::Mutex`): never call back into the same
/// box, or into the host, from inside `body`.
final class Locked<Value>: @unchecked Sendable {
    private var value: Value
    private let lock = NSLock()

    init(_ value: Value) {
        self.value = value
    }

    func withLock<R>(_ body: (inout Value) throws -> R) rethrows -> R {
        lock.lock()
        defer { lock.unlock() }
        return try body(&value)
    }
}

extension Locked where Value: Sendable {
    /// A copy of the current value.
    func get() -> Value { withLock { $0 } }

    /// Replaces the value, returning the previous one (an atomic `swap`).
    @discardableResult
    func swap(_ new: Value) -> Value {
        withLock { value in
            let old = value
            value = new
            return old
        }
    }
}

/// The engine's log. KudzuServices owns the file log; this module only needs `os.Logger`.
enum EngineLog {
    static let logger = Logger(subsystem: Bundle.main.bundleIdentifier ?? "sh.saeed.kudzu", category: "engine")
}
