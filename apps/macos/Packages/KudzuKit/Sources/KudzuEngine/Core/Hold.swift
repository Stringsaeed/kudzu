// FIFO of physical input withheld while an insertion (or its replay) is in flight.
// Port of `kudzu-engine/src/hold.rs`.

/// Whether incoming input should be withheld.
enum HoldState: Hashable, Sendable {
    case idle
    case inserting
}

/// Holds keystrokes typed during an insertion so they land after it, in order.
struct HoldReplayQueue<Element> {
    private(set) var state: HoldState = .idle
    private var elements: [Element] = []

    init() {}

    /// Starts holding, preserving any input still waiting for replay.
    mutating func begin() {
        state = .inserting
    }

    /// Accepts `element` while holding. Returns false when idle (the element must pass through).
    @discardableResult
    mutating func enqueue(_ element: Element) -> Bool {
        guard state == .inserting else { return false }
        elements.append(element)
        return true
    }

    /// Takes one element to replay; stays holding until a read finds the queue empty.
    mutating func next() -> Element? {
        guard !elements.isEmpty else {
            state = .idle
            return nil
        }
        return elements.removeFirst()
    }

    /// Releases everything in order and returns to idle.
    mutating func drain() -> [Element] {
        state = .idle
        defer { elements.removeAll() }
        return elements
    }

    /// Number of held elements.
    var count: Int { elements.count }
}
