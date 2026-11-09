import Foundation
import KudzuCore

/// Something the backend pushes to every window: the port of `Events` in `lib/backend.ts`
/// (`kudzu://library`, `kudzu://settings`, …), with typed payloads.
public enum BackendEvent: Hashable, Sendable {
    /// `kudzu://library`: the whole library after any change.
    case library(Library)
    /// `kudzu://settings`.
    case settings(Settings)
    /// `kudzu://status`: engine status (pause, permissions, Secure Input, tap mode).
    case status(EngineStatus)
    /// `kudzu://update`.
    case update(UpdateState)
    /// `kudzu://sync`.
    case sync(SyncStatus)
    /// `kudzu://clipboard`: a change to the clipboard history.
    case clipboard(ClipEvent)
    /// `kudzu://navigate`: show a view and/or select a snippet.
    case navigate(Navigate)
    /// `kudzu://new-snippet`: start a snippet (from the palette, tray or clipboard).
    case newSnippet(NewSnippet)
    /// `kudzu://form-request`: the form panel should show this form.
    case formRequest(FormPayload)
    /// `kudzu://palette-show`: the palette opened over `target`.
    case paletteShow(target: AppInfo?)
    /// `kudzu://clipboard-show`: the clipboard picker opened over `target`.
    case clipboardShow(target: AppInfo?, canPaste: Bool?)
    /// `kudzu://permission-show`: the permission companion should guide `kind`.
    case permissionShow(PermissionKind)
    /// `kudzu://command`: declared by the TypeScript `Events` but never emitted by the Tauri app.
    case command(String)

    /// `{ snippetId?, view? }` of `kudzu://navigate`.
    public struct Navigate: Codable, Hashable, Sendable {
        public var snippetId: UUID?
        public var view: Route?
        public init(snippetId: UUID? = nil, view: Route? = nil) {
            self.snippetId = snippetId
            self.view = view
        }
    }

    /// `{ content?, name?, trigger? }` of `kudzu://new-snippet`.
    public struct NewSnippet: Codable, Hashable, Sendable {
        public var content: String?
        public var name: String?
        public var trigger: String?
        public init(content: String? = nil, name: String? = nil, trigger: String? = nil) {
            self.content = content
            self.name = name
            self.trigger = trigger
        }
    }
}

/// Fans events out to any number of `AsyncStream` subscribers (the Tauri event bus).
public final class EventBroadcaster: Sendable {
    // `Mutex` would need macOS 15; a lock-protected dictionary keeps 14 support.
    private let state = LockedState()

    public init() {}

    /// A new subscription that receives every event sent after this call.
    public func subscribe() -> AsyncStream<BackendEvent> {
        let (stream, continuation) = AsyncStream<BackendEvent>.makeStream(bufferingPolicy: .unbounded)
        let id = state.add(continuation)
        continuation.onTermination = { [state] _ in state.remove(id) }
        return stream
    }

    public func send(_ event: BackendEvent) {
        for continuation in state.all() { continuation.yield(event) }
    }

    /// The continuations, behind an `NSLock`.
    private final class LockedState: @unchecked Sendable {
        // @unchecked: every access to `continuations`/`next` holds `lock`.
        private let lock = NSLock()
        private var continuations: [Int: AsyncStream<BackendEvent>.Continuation] = [:]
        private var next = 0

        func add(_ c: AsyncStream<BackendEvent>.Continuation) -> Int {
            lock.withLock {
                next += 1
                continuations[next] = c
                return next
            }
        }

        func remove(_ id: Int) {
            lock.withLock { _ = continuations.removeValue(forKey: id) }
        }

        func all() -> [AsyncStream<BackendEvent>.Continuation] {
            lock.withLock { Array(continuations.values) }
        }
    }
}
