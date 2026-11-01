/// What the app must do to revert the expansion that just happened.
public struct UndoAction: Hashable, Sendable {
    /// Characters to backspace: everything inserted, plus the re-emitted separator if any.
    public let deleteCount: Int
    /// Text to type afterwards: the trigger exactly as originally typed, plus its separator.
    public let retype: String

    public init(deleteCount: Int, retype: String) {
        self.deleteCount = deleteCount
        self.retype = retype
    }
}

/// Remembers the last expansion so an immediately following backspace can revert it.
///
/// The backspace that triggers the undo must be **swallowed** by the app (it is accounted for in
/// `UndoAction.deleteCount`). Synthesized undo output must not be fed back as input.
public struct UndoTracker: Hashable, Sendable {
    private struct Pending: Hashable, Sendable {
        var insertedCount: Int
        var typedTrigger: String
        var separator: String?
    }

    private var pending: Pending?

    public init() {}

    /// Whether the next backspace would undo an expansion.
    public var hasPending: Bool { pending != nil }

    /// Records an expansion. `insertedCount` is the number of characters (grapheme clusters) the
    /// app inserted, not counting a separator it re-emitted afterwards.
    public mutating func recordExpansion(of match: TriggerMatch, insertedCount: Int) {
        pending = Pending(insertedCount: insertedCount, typedTrigger: match.typedTrigger, separator: match.separator)
    }

    /// Feeds the next input. Returns an `UndoAction` for a backspace directly after an expansion;
    /// any input clears the pending undo.
    public mutating func consume(_ input: KeyInput) -> UndoAction? {
        defer { pending = nil }
        guard let pending, input == .backspace else { return nil }
        return UndoAction(
            deleteCount: pending.insertedCount + (pending.separator?.count ?? 0),
            retype: pending.typedTrigger + (pending.separator ?? "")
        )
    }

    /// Forgets any pending undo.
    public mutating func clear() {
        pending = nil
    }
}
