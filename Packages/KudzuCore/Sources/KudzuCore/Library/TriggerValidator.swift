import Foundation

/// Why a trigger typed into the editor was rejected.
public enum TriggerIssue: Error, Hashable, Sendable {
    /// Nothing was entered.
    case empty
    /// Only spaces, tabs or newlines were entered.
    case whitespaceOnly
    /// The snippet already has this trigger.
    case duplicate
    /// Longer than `TriggerValidator.maxLength`, so it could never fire.
    case tooLong(length: Int)

    /// A short sentence suitable for inline display.
    public var message: String {
        switch self {
        case .empty: "Enter a trigger first."
        case .whitespaceOnly: "A trigger can't be only spaces."
        case .duplicate: "This snippet already has that trigger."
        case .tooLong(let length): "Triggers can be at most \(TriggerValidator.maxLength) characters; this one has \(length)."
        }
    }
}

/// Validates triggers before they are added to a snippet.
public enum TriggerValidator {
    /// Longest usable trigger: the key buffer holds this many characters, so longer ones can never match.
    public static let maxLength = KeyBuffer.defaultCapacity

    /// Returns the cleaned trigger (surrounding whitespace removed) or the reason it was rejected.
    ///
    /// Length is counted in grapheme clusters, like `KeyBuffer`. Duplicates are compared
    /// case-insensitively unless `caseSensitive` is true (snippets using exact case matching).
    public static func validate(
        _ raw: String,
        existing: [String],
        caseSensitive: Bool = false
    ) -> Result<String, TriggerIssue> {
        if raw.isEmpty { return .failure(.empty) }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return .failure(.whitespaceOnly) }
        if trimmed.count > maxLength { return .failure(.tooLong(length: trimmed.count)) }
        let isDuplicate = existing.contains { other in
            caseSensitive ? other == trimmed : other.caseInsensitiveCompare(trimmed) == .orderedSame
        }
        if isDuplicate { return .failure(.duplicate) }
        return .success(trimmed)
    }

    /// Splits pasted or typed text on whitespace into candidate triggers.
    public static func tokens(from text: String) -> [String] {
        text.split(whereSeparator: \.isWhitespace).map(String.init)
    }
}
