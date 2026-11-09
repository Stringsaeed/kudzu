import CoreGraphics
import Foundation
import KudzuCore
@testable import KudzuEngine
import Testing

@Suite struct RuntimeTests {
    private final class Host: EngineHost {
        func statusChanged(_ status: EngineStatus) {}
        func expanded(_ event: ExpansionEvent) {}
        func formRequested(_ request: FormRequest) {}
    }

    @Test func tapDisableNotificationsInvalidateTheInsertion() throws {
        let runtime = EngineRuntime(host: Host())
        defer { runtime.shutdown() }
        let event = try #require(CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: true))
        for kind in [CGEventType.tapDisabledByTimeout, .tapDisabledByUserInput] {
            runtime.core.withLock { core in
                core.busy = true
                core.startHold()
                _ = core.matcher.handle(.character("prefix"), bundleID: nil)
            }
            let generation = runtime.core.withLock { $0.generation }
            #expect(!runtime.receive(kind, event, mode: .active))
            runtime.core.withLock { core in
                #expect(core.generation == generation + 1)
                #expect(!core.busy)
                #expect(core.held.state == .idle)
                #expect(core.matcher.buffer.isEmpty)
            }
        }
    }

    @Test func syntheticKeysNeverRematchOrClearTheTypedBuffer() throws {
        let runtime = EngineRuntime(host: Host())
        defer { runtime.shutdown() }
        let event = try #require(CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: true))
        event.setIntegerValueField(.eventSourceUserData, value: Keys.syntheticMarker)
        runtime.core.withLock { _ = $0.matcher.handle(.character("prefix"), bundleID: nil) }
        #expect(!runtime.receive(.keyDown, event, mode: .active))
        #expect(runtime.core.withLock { $0.matcher.buffer.string } == "prefix")
    }

    @Test func chainedHoldsKeepTheOriginalDeadline() {
        var core = Core()
        core.startHold()
        core.holdSince = RuntimeTiming.now() - 1.1
        core.busy = true
        core.startHold()
        #expect(core.holdExpired)
        let generation = core.generation
        _ = core.invalidate()
        #expect(core.generation == generation + 1)
        #expect(!core.busy)
        #expect(!core.holdExpired)
        #expect(core.holdSince == nil)
        #expect(core.held.state == .idle)
    }

    @Test func replayStaysHeldUntilTheEmptyRead() throws {
        var core = Core()
        let event = try #require(CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: true))
        let held = try #require(HeldEvent.copy(event, keyDown: true))
        core.startHold()
        let accepted = core.held.enqueue(held)
        #expect(accepted)
        core.busy = true
        let busyReplay = core.nextReplay()
        #expect(busyReplay == nil)
        #expect(core.held.count == 1)
        core.busy = false
        let first = core.nextReplay()
        #expect(first === held)
        #expect(core.held.state == .inserting)
        #expect(core.holdSince != nil)
        let empty = core.nextReplay()
        #expect(empty == nil)
        #expect(core.held.state == .idle)
        #expect(core.holdSince == nil)
    }

    @Test func anOlderReloadCannotReplaceANewerLibrary() {
        let runtime = EngineRuntime(host: Host())
        let started = DispatchSemaphore(value: 0)
        let releaseOlder = DispatchSemaphore(value: 0)
        let finished = DispatchSemaphore(value: 0)
        defer {
            releaseOlder.signal()
            runtime.shutdown()
        }
        runtime.beforeLibraryInstall.swap { revision in
            guard revision == 1 else { return }
            started.signal()
            releaseOlder.wait()
        }
        let older = EngineLibrary(snippets: [Snippet(name: "Old", triggers: [";old"], content: "old")])
        let newer = EngineLibrary(snippets: [Snippet(name: "New", triggers: [";new"], content: "new")])
        Thread {
            runtime.reload(older, revision: 1)
            finished.signal()
        }.start()
        #expect(started.wait(timeout: .now() + 5) == .success)
        runtime.reload(newer, revision: 2)
        releaseOlder.signal()
        #expect(finished.wait(timeout: .now() + 5) == .success)
        let installed = runtime.core.withLock { core in
            (core.appliedRevision, core.library.snippets.values.map(\.content))
        }
        #expect(installed.0 == 2)
        #expect(installed.1 == ["new"])

        runtime.reload(newer, revision: 4)
        #expect(runtime.core.withLock { $0.appliedRevision } == 4)
        #expect(runtime.core.withLock { $0.matcher.buffer.isEmpty })
        runtime.reload(older, revision: 3)
        #expect(runtime.core.withLock { $0.appliedRevision } == 4)
        #expect(runtime.core.withLock { $0.library.snippets.values.map(\.content) } == ["new"])
        runtime.reload(older)
        #expect(runtime.core.withLock { $0.appliedRevision } == 4)
        #expect(runtime.core.withLock { $0.library.snippets.values.map(\.content) } == ["old"])
    }
}
