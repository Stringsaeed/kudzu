import Foundation

public func findConflicts(snippets: [Snippet], groups: [Group]) -> [TriggerConflict] { TriggerConflicts.find(snippets: snippets, groups: groups) }
public func validateTrigger(_ raw: String, existing: [String], caseSensitive: Bool = false) -> Result<String, TriggerIssue> { TriggerValidator.validate(raw, existing: existing, caseSensitive: caseSensitive) }
public func triggerTokens(_ text: String) -> [String] { TriggerValidator.tokens(from: text) }
public typealias ConflictKind = TriggerConflict.Kind
