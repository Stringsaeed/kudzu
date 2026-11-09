// The typing rules of `TriggerInput` (`trigger-input.tsx`), separate from the view so they can be
// tested: whitespace or Enter adds, Tab adds when there is something to add, Backspace on an empty
// field takes the last trigger back for editing, Escape clears the draft, the default prefix is
// pre-filled on focus when the snippet has no triggers yet.

import KudzuCore

struct TriggerEntry: Equatable {
    /// The text in the field.
    var draft = ""
    /// The validation message shown under the field (`TriggerIssue.message`).
    var error: String?

    /// Validates every whitespace-separated part of `raw` and returns the new trigger list, or
    /// `nil` when nothing changed (blank input) or a part was rejected (then `error` is set and
    /// none are added).
    mutating func commit(_ raw: String? = nil, triggers: [String], caseSensitive: Bool) -> [String]? {
        let raw = raw ?? draft
        guard !TriggerValidator.trimmed(raw).isEmpty else { return nil }
        var next = triggers
        for part in TriggerValidator.tokens(from: raw) {
            switch TriggerValidator.validate(part, existing: next, caseSensitive: caseSensitive) {
            case .success(let trigger): next.append(trigger)
            case .failure(let issue):
                error = issue.message
                return nil
            }
        }
        draft = ""
        error = nil
        return next
    }

    /// `onFocus`: an empty field of a snippet without triggers starts with the default prefix.
    mutating func focused(triggers: [String], prefix: String) {
        if draft.isEmpty, triggers.isEmpty, !prefix.isEmpty { draft = prefix }
    }

    /// `onBlur`: drops an untouched prefix, commits anything else.
    mutating func blurred(triggers: [String], prefix: String, caseSensitive: Bool) -> [String]? {
        if draft == prefix {
            draft = ""
            return nil
        }
        return commit(triggers: triggers, caseSensitive: caseSensitive)
    }

    /// `onChange` / `onPaste`: typing whitespace after text commits; pasted text containing
    /// whitespace commits every part; leading whitespace is dropped. (A native field has no separate
    /// paste event, but whitespace inside the text can only arrive by pasting: typed spaces commit.)
    mutating func typed(_ value: String, triggers: [String], caseSensitive: Bool) -> [String]? {
        error = nil
        let trimmed = TriggerValidator.trimmed(value)
        let endsInSpace = value.unicodeScalars.last?.properties.isWhitespace ?? false
        let innerSpace = trimmed.unicodeScalars.contains { $0.properties.isWhitespace }
        if !trimmed.isEmpty, endsInSpace || innerSpace {
            // A rejected part leaves the field as it was (the web keeps the controlled value).
            return commit(value, triggers: triggers, caseSensitive: caseSensitive)
        }
        draft = trimmed
        return nil
    }

    /// Whether Enter / Tab have something to add (not blank, not just the prefix).
    func canCommit(prefix: String) -> Bool {
        !TriggerValidator.trimmed(draft).isEmpty && draft != prefix
    }

    /// Backspace in an empty field: removes the last trigger and puts it back in the field.
    mutating func backspace(triggers: [String]) -> [String]? {
        guard draft.isEmpty, let last = triggers.last else { return nil }
        draft = last
        return Array(triggers.dropLast())
    }

    /// Escape with a draft clears it (and is consumed); otherwise Escape passes on.
    mutating func escape() -> Bool {
        guard !draft.isEmpty else { return false }
        draft = ""
        error = nil
        return true
    }

    static func placeholder(triggers: [String], prefix: String) -> String {
        triggers.isEmpty ? "\(prefix)trigger" : "Add another…"
    }
}
