@testable import KudzuEngine
import Testing

// Port of the tests in `kudzu-engine/src/hold.rs`.
@Suite struct HoldTests {
    @Test func idleQueuePassesInputThrough() {
        var q = HoldReplayQueue<Int>()
        let r1 = q.enqueue(1)
        #expect(r1 == false)
        #expect(q.state == .idle)
    }

    @Test func holdsAndReplaysInOrderThenGoesIdle() {
        var q = HoldReplayQueue<Int>()
        q.begin()
        let r2 = q.enqueue(1)
        #expect(r2)
        let r3 = q.enqueue(2)
        #expect(r3)
        let r4 = q.next()
        #expect(r4 == 1)
        // Still holding: a key typed during replay queues behind the rest.
        let r5 = q.enqueue(3)
        #expect(r5)
        let r6 = q.next()
        #expect(r6 == 2)
        let r7 = q.next()
        #expect(r7 == 3)
        #expect(q.state == .inserting)
        let r8 = q.next()
        #expect(r8 == nil)
        #expect(q.state == .idle)
    }

    @Test func beginDuringReplayKeepsRemainingInput() {
        var q = HoldReplayQueue<Character>()
        q.begin()
        let r9 = q.enqueue("a")
        #expect(r9)
        let r10 = q.enqueue("b")
        #expect(r10)
        let r11 = q.next()
        #expect(r11 == "a")
        // 'a' completed another trigger: a new insertion starts with 'b' still queued.
        q.begin()
        let r12 = q.enqueue("c")
        #expect(r12)
        #expect(q.count == 2)
        let r13 = q.drain()
        #expect(r13 == ["b", "c"])
        #expect(q.state == .idle)
        #expect(q.count == 0)
    }
}
