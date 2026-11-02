import Foundation

/// Merges editor-owned changes into the latest stored version without overwriting concurrent edits.
public enum SnippetDraft {
    /// A save cannot safely update a deleted record or a field changed differently on both sides.
    public enum Conflict: Error, LocalizedError, Equatable {
        case deleted
        case changed(String)

        /// Explains how to recover without overwriting the stored version.
        public var errorDescription: String? {
            switch self {
            case .deleted: "This snippet was deleted. Your draft is retained; copy it or discard the changes."
            case .changed(let field): "The stored \(field) changed while you were editing. Your draft is retained; copy it or discard to load the stored version."
            }
        }
    }

    /// Applies only fields changed from the baseline, refusing conflicting changes and missing records.
    public static func merge(_ draft: SnippetRecord, baseline: SnippetRecord, latest: SnippetRecord?) throws -> SnippetRecord {
        guard var result = latest else { throw Conflict.deleted }
        func merge<Value: Equatable>(_ path: WritableKeyPath<SnippetRecord, Value>, _ name: String) throws {
            guard draft[keyPath: path] != baseline[keyPath: path] else { return }
            guard result[keyPath: path] == baseline[keyPath: path] || result[keyPath: path] == draft[keyPath: path] else {
                throw Conflict.changed(name)
            }
            result[keyPath: path] = draft[keyPath: path]
        }
        try merge(\.name, "name")
        try merge(\.triggers, "triggers")
        try merge(\.content, "content")
        try merge(\.groupID, "group")
        try merge(\.fireMode, "fire mode")
        try merge(\.caseMode, "case mode")
        try merge(\.insertion, "insertion method")
        try merge(\.rule, "app rules")
        try merge(\.isFavorite, "favorite status")
        try merge(\.isEnabled, "enabled status")
        return result
    }
}
