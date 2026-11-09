/// What the matcher wants the app to do after a keystroke.
public enum MatcherEvent: Hashable, Sendable {
    /// A trigger fired: delete it, insert the expansion, then call `ExpansionMatcher.didExpand`.
    case expand(TriggerMatch)
    /// Backspace right after an expansion: swallow the backspace and revert the expansion.
    case undo(UndoAction)
}

/// Single entry point for the event tap: `KeyBuffer` + `TriggerIndex` + `UndoTracker`.
///
/// A value type: keep it in one place (an actor or a lock-guarded box) and call `handle` for
/// every real keystroke. **The app must not feed its own synthesized events (backspaces, typed
/// expansion, pasted text, retyped undo text) back into `handle`**; the matcher already accounts
/// for them via `didExpand(match:insertedCount:)` and the returned `UndoAction`.
public struct ExpansionMatcher: Sendable {
    public private(set) var buffer: KeyBuffer
    private var index: TriggerIndex
    private var undoTracker = UndoTracker()

    /// An inert matcher with no triggers; call `reload` to load a library.
    public init(bufferCapacity: Int = KeyBuffer.defaultCapacity) {
        self.buffer = KeyBuffer(capacity: bufferCapacity)
        self.index = .empty
    }

    public init(
        snippets: [Snippet],
        groups: [Group],
        config: MatchingConfig,
        bufferCapacity: Int = KeyBuffer.defaultCapacity
    ) {
        self.buffer = KeyBuffer(capacity: bufferCapacity)
        self.index = TriggerIndex(snippets: snippets, groups: groups, config: config)
    }

    /// Processes one real keystroke typed in the app with `bundleID` (nil if unknown).
    ///
    /// A backspace directly after `didExpand` returns `.undo` (and is not applied to the buffer,
    /// since the app swallows it). Otherwise the buffer is updated and a completed trigger returns `.expand`.
    public mutating func handle(_ input: KeyInput, bundleID: String?) -> MatcherEvent? {
        if let undo = undoTracker.consume(input) {
            // Mirror the document after the app retypes the trigger; no matching is attempted.
            buffer.reset()
            buffer.append(undo.retype)
            return .undo(undo)
        }
        buffer.apply(input)
        return index.match(buffer: buffer, lastInput: input, bundleID: bundleID).map(MatcherEvent.expand)
    }

    /// Call after inserting the expansion for `match`. `insertedCount` is the number of characters
    /// (grapheme clusters) inserted, excluding a separator re-emitted afterwards. Clears the buffer
    /// (the inserted text is not fed back) and arms undo.
    public mutating func didExpand(match: TriggerMatch, insertedCount: Int) {
        buffer.reset()
        undoTracker.recordExpansion(of: match, insertedCount: insertedCount)
    }

    /// Replaces the library, rebuilding the index synchronously. Pending undo is dropped.
    public mutating func reload(snippets: [Snippet], groups: [Group], config: MatchingConfig) {
        replaceIndex(TriggerIndex(snippets: snippets, groups: groups, config: config))
    }

    /// Swaps in an index built elsewhere (e.g. off the event-tap thread). Pending undo is dropped.
    public mutating func replaceIndex(_ newIndex: TriggerIndex) {
        index = newIndex
        undoTracker.clear()
    }
    /// A matcher over an immutable index.
    public init(index: TriggerIndex) { self.buffer = KeyBuffer(); self.index = index }
    /// Clears the key buffer and pending undo (click, app switch, pause…).
    public mutating func reset() { buffer.reset(); undoTracker.clear() }
}
