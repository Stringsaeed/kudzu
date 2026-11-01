import Foundation

/// Last submitted form values, stored separately for each snippet.
public struct FormMemory: Codable, Equatable, Sendable {
    private var values: [UUID: [String: String]] = [:]

    /// Creates empty form memory.
    public init() {}

    /// Chooses valid remembered values, then defaults, then the first choice or empty text.
    public func initialValues(for snippetID: UUID, fields: [FormFieldSpec]) -> [String: String] {
        var result: [String: String] = [:]
        for field in fields {
            let candidates = [values[snippetID]?[field.name], field.defaultValue]
            result[field.name] = candidates.compactMap { $0 }.first {
                field.kind != .choice || field.options.contains($0)
            } ?? (field.kind == .choice ? field.options.first : nil) ?? ""
        }
        return result
    }

    /// Records a submission, bounding storage to 200 snippets.
    public mutating func record(_ submitted: [String: String], for snippetID: UUID) {
        if values[snippetID] == nil, values.count >= 200, let victim = values.keys.sorted(by: { $0.uuidString < $1.uuidString }).first {
            values.removeValue(forKey: victim)
        }
        values[snippetID] = submitted
    }

    /// Removes memory for snippets that no longer exist.
    public mutating func retainSnippets(_ ids: Set<UUID>) {
        values = values.filter { ids.contains($0.key) }
    }
}
