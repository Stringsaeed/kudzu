import Foundation

/// Synchronous producers enqueue work; shutdown closes admission and awaits the tail.
public final class AsyncWorkQueue: @unchecked Sendable {
    private let lock = NSLock()
    private var tail: Task<Void, Never>?
    private var closed = false
    private var acceptedCount = 0
    var testingAcceptedCount: Int { lock.withLock { acceptedCount } }
    public init() {}
    @discardableResult public func enqueue(_ work: @escaping @Sendable () async -> Void) -> Bool {
        lock.withLock {
            guard !closed else { return false }
            acceptedCount += 1
            let previous = tail
            tail = Task { await previous?.value; await work() }
            return true
        }
    }
    public func drain() async {
        let task = lock.withLock { closed = true; return tail }
        await task?.value
    }
}
