/// FIFO input held while an insertion or its replay is in flight.
public struct HoldReplayQueue<Element> {
    /// Whether incoming input should be withheld.
    public enum State: Sendable { case idle, inserting }
    /// Current hold state.
    public private(set) var state: State = .idle
    private var elements: [Element] = []
    private var head = 0

    /// Creates an idle queue.
    public init() {}

    /// Starts holding, preserving any remaining replay input.
    public mutating func begin() { state = .inserting }

    /// Returns true when the input was accepted and must be swallowed.
    public mutating func enqueue(_ element: Element) -> Bool {
        guard state == .inserting else { return false }
        elements.append(element)
        return true
    }

    /// Takes one replay event, remaining in hold mode until the next empty read.
    public mutating func next() -> Element? {
        guard head < elements.count else { _ = drain(); return nil }
        defer { head += 1 }
        return elements[head]
    }

    /// Releases all remaining input in order and returns to idle.
    public mutating func drain() -> [Element] {
        let result = Array(elements.dropFirst(head))
        elements.removeAll(keepingCapacity: true)
        head = 0
        state = .idle
        return result
    }

    /// Releases held input when the insertion deadline expires.
    public mutating func timeout() -> [Element] { drain() }
}

extension HoldReplayQueue: Sendable where Element: Sendable {}
