import Foundation
import KudzuCore

// State and messages of the engine runtime. Port of the types at the top of `macos/engine.rs`.

enum RuntimeTiming {
    /// Physical input is never held longer than this, whatever the target app does.
    static let holdLimit: TimeInterval = 1.0
    /// How long to wait for a reactivated app to become frontmost.
    static let activationTimeout: TimeInterval = 0.5
    /// Lets focus changes and flushed input reach the target before we act on it.
    static let settle: TimeInterval = 0.05
    /// Listen-only: let the final trigger key reach the document before deleting it.
    static let listenOnlySettle: TimeInterval = 0.03
    static let pollTick: TimeInterval = 0.1
    static let statusEveryTicks: UInt32 = 10

    /// Monotonic seconds (Rust `Instant`).
    static func now() -> TimeInterval { ProcessInfo.processInfo.systemUptime }
}

/// Which guard an insertion re-checks between steps.
enum InsertGuard {
    /// A typed trigger: blocked in our own windows and while suppressed.
    case typed
    /// Something the user explicitly asked for (form submit, palette).
    case explicit
}

enum FormKind: Sendable {
    /// Typed trigger, still in the document (as typed, with any separator).
    case typed(TriggerMatch)
    /// Palette insert: nothing to delete.
    case palette
}

struct PendingForm: Sendable {
    var token: UInt64
    var snippetId: UUID
    var kind: FormKind
    var target: AppInfo?
    /// Typing reached the target after the trigger; deleting it now would delete the wrong text.
    var interrupted: Bool
}

struct Matched: Sendable {
    var generation: UInt64
    var event: MatcherEvent
    var mode: PlanMode
    var undoCaret: Int
    var swallowed: String?
    var pid: pid_t?
}

enum Job: Sendable {
    case matched(Matched)
    case submit(token: UInt64, values: [String: String])
    case insert(snippetId: UUID, target: AppInfo?)
    case replay
}

/// State the tap callback touches. Keep every operation on it short.
struct Core {
    var matcher = ExpansionMatcher()
    var library = LibrarySnapshot()
    var held = HoldReplayQueue<HeldEvent>()
    var holdSince: TimeInterval?
    /// Bumped whenever in-flight work must stop (click, pause, deadline, new insertion).
    var generation: UInt64 = 0
    /// Highest library revision installed by ``EngineRuntime/reload(_:revision:)``.
    /// Unversioned reloads do not change it. Not the hold `generation` above.
    var appliedRevision: UInt64 = 0
    /// An insertion is in flight.
    var busy = false
    var listenOnlyInterrupted = false
    var preparingForm = false
    /// A key reached the target while the form was being prepared.
    var preparingInterrupted = false
    var pendingForm: PendingForm?
    var nextFormToken: UInt64 = 1
    /// Left-arrow presses after the last expansion's cursor placement (undo moves back first).
    var undoCaretLeft = 0
    var frontmostPid: pid_t?
    var frontmostBundle: String?

    mutating func resetMatcher() {
        matcher.reset()
        undoCaretLeft = 0
    }

    mutating func startHold() {
        if held.state != .inserting {
            holdSince = RuntimeTiming.now()
        }
        held.begin()
    }

    /// Stops in-flight work and returns held input, to be posted unmatched.
    mutating func invalidate() -> [HeldEvent] {
        generation &+= 1
        busy = false
        preparingForm = false
        holdSince = nil
        resetMatcher()
        return held.drain()
    }

    /// Next held event to replay once nothing is in flight.
    mutating func nextReplay() -> HeldEvent? {
        if busy {
            return nil
        }
        let next = held.next()
        if next == nil {
            holdSince = nil
        }
        return next
    }

    var holdExpired: Bool {
        guard held.state == .inserting, let since = holdSince else { return false }
        return RuntimeTiming.now() - since > RuntimeTiming.holdLimit
    }
}

/// The Rust engine's atomics, behind one lock.
struct Flags: Sendable {
    var running = false
    var paused = false
    var suppressed = false
    var secureInput = false
    var postEvent = false
    var inputMonitoring = false
    var tapMode = TapMode.off
    var pollEpoch: UInt64 = 0
}

/// Effects of a tap callback, performed after `core` is unlocked.
struct Effects {
    var postUnmatched: [HeldEvent] = []
    var job: Job?
    var replay = false
}

/// Why an insertion step failed.
enum InsertError: Error, CustomStringConvertible {
    case key(KeyError)
    case paste(PasteFailure)

    var description: String {
        switch self {
        case .key(let error): "keystroke: \(error)"
        case .paste(let error): "paste: \(error)"
        }
    }
}

/// The worker's job queue (Rust `mpsc::channel`): FIFO, blocking receive, drained after close.
///
/// `@unchecked Sendable`: `jobs` and `closed` are only touched while `condition` is locked.
final class JobQueue: @unchecked Sendable {
    private let condition = NSCondition()
    private var jobs: [Job] = []
    private var closed = false

    func send(_ job: Job) {
        condition.lock()
        defer { condition.unlock() }
        guard !closed else { return }
        jobs.append(job)
        condition.signal()
    }

    /// Stops accepting jobs; the receiver finishes what is queued, then ``next()`` returns nil.
    func close() {
        condition.lock()
        closed = true
        condition.broadcast()
        condition.unlock()
    }

    /// Blocks until a job arrives; nil once closed and empty.
    func next() -> Job? {
        condition.lock()
        defer { condition.unlock() }
        while jobs.isEmpty && !closed {
            condition.wait()
        }
        return jobs.isEmpty ? nil : jobs.removeFirst()
    }
}
