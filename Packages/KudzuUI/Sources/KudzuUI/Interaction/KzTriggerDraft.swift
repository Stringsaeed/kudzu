import Foundation

/// What happens to the uncommitted text in the trigger field.
///
/// A valid draft commits on Return, Tab, Shift-Tab, and when the field resigns. An invalid one stays
/// in the field so it can be corrected. Escape drops only that uncommitted text. Marked text belongs
/// to the input method and is left alone.
public enum KzTriggerDraft {
    public enum Command: Equatable, Sendable {
        case newline
        case tab
        case backtab
        case escape
        case deleteBackward
    }

    public enum Result: Equatable, Sendable {
        /// The field or the input method handles the key.
        case passthrough
        /// Commit when the text is valid; otherwise keep it and show why.
        case commit
        /// Move focus. The draft is empty, so there is nothing to commit.
        case move(forward: Bool)
        /// Commit, then move focus when the commit succeeded.
        case commitAndMove(forward: Bool)
        /// Keep the text where it is and show the rejection.
        case reject
        /// Clear the uncommitted text and its error.
        case cancel
        /// The draft is empty, so Backspace removes the last saved trigger.
        case removeLast
    }

    public enum Finish: Equatable, Sendable {
        /// Leave the draft as it is (composition in progress, or Escape already handled it).
        case keep
        /// The field is empty; clear a stale error.
        case clearError
        /// Commit the valid text.
        case commit
        /// Keep the invalid text and show the rejection.
        case reject
    }

    /// `isValid` is ignored when `text` is empty or the input method is composing.
    public static func handle(_ command: Command, text: String, isComposing: Bool, isValid: Bool) -> Result {
        if isComposing { return .passthrough }
        let pending = !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        switch command {
        case .newline:
            return .commit
        case .tab, .backtab:
            let forward = command == .tab
            if !pending { return .move(forward: forward) }
            return isValid ? .commitAndMove(forward: forward) : .reject
        case .escape:
            return pending ? .cancel : .passthrough
        case .deleteBackward:
            return text.isEmpty ? .removeLast : .passthrough
        }
    }

    /// What to do when the field resigns: commit valid text, keep invalid text, and don't commit over a cancel.
    public static func finish(text: String, isComposing: Bool, cancelled: Bool, isValid: Bool) -> Finish {
        if isComposing || cancelled { return .keep }
        let pending = !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        if !pending { return .clearError }
        return isValid ? .commit : .reject
    }
}
