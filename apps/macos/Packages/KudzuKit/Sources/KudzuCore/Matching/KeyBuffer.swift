/// Rolling buffer of the most recently typed grapheme clusters.
///
/// Value type: copies are independent (copy-on-write), and each keystroke costs
/// an append plus, once full, a move of at most `capacity` elements.
public struct KeyBuffer: Hashable, Sendable {
    /// Default capacity in grapheme clusters. Triggers longer than this can never fire.
    public static let defaultCapacity = 64

    /// Maximum number of grapheme clusters retained.
    public let capacity: Int
    /// Buffered characters, oldest first.
    public private(set) var characters: ContiguousArray<Character>

    public init(capacity: Int = KeyBuffer.defaultCapacity) {
        self.capacity = max(1, capacity)
        self.characters = []
        self.characters.reserveCapacity(self.capacity + 4)
    }

    public var count: Int { characters.count }
    public var isEmpty: Bool { characters.isEmpty }
    public var last: Character? { characters.last }
    /// The buffered text as a string.
    public var string: String { String(characters) }

    /// Applies one input: appends text, drops the last character, or clears.
    public mutating func apply(_ input: KeyInput) {
        switch input {
        case .character(let text): append(text)
        case .backspace: removeLast()
        case .reset: reset()
        }
    }

    /// Appends typed text, merging a leading combining mark / joiner / second
    /// regional indicator into the previous character so the buffer always holds
    /// whole grapheme clusters, as a backspace in the target app would see them.
    public mutating func append(_ text: String) {
        guard let firstScalar = text.unicodeScalars.first else { return }
        // Fast path: only a scalar at or above U+0300 (or LF after CR) can extend the previous cluster.
        if firstScalar.value >= 0x300 || firstScalar == "\n", let previous = characters.last {
            let joined = String(previous) + text
            if let head = joined.first, head.unicodeScalars.count > previous.unicodeScalars.count {
                characters.removeLast()
                characters.append(contentsOf: joined)
                trim()
                return
            }
        }
        characters.append(contentsOf: text)
        trim()
    }

    /// Drops the last character, if any.
    public mutating func removeLast() {
        if !characters.isEmpty { characters.removeLast() }
    }

    /// Clears the buffer.
    public mutating func reset() {
        characters.removeAll(keepingCapacity: true)
    }

    private mutating func trim() {
        let overflow = characters.count - capacity
        if overflow > 0 { characters.removeFirst(overflow) }
    }
}
