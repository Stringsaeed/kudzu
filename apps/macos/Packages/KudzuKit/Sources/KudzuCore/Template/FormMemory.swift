import Foundation

/// Last submitted form values, stored separately for each snippet.
public struct FormMemory: Codable, Equatable, Sendable {
    public var values: [UUID: [String: String]] = [:]

    private enum CodingKeys: String, CodingKey { case values }
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let strings = try container.decode([String: [String: String]].self, forKey: .values)
        values = Dictionary(strings.compactMap { k, v in UUID(uuidString: k).map { ($0, v) } }, uniquingKeysWith: { a, _ in a })
    }
    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(Dictionary(uniqueKeysWithValues: values.map { ($0.key.uuidString.lowercased(), $0.value) }), forKey: .values)
    }
    public static let capacity = 200
    /// Creates empty form memory.
    public init() {}

    /// Chooses valid remembered values, then defaults, then the first choice or empty text.
    ///
    /// Names and options compare by bytes, as in `kudzu-core`. The result is a `[String: String]`, so of two
    /// canonically equivalent field names (`é`, `e` + U+0301) it can hold only the first.
    public func initialValues(for snippetID: UUID, fields: [FormFieldSpec]) -> [String: String] {
        var result: [String: String] = [:]
        for field in fields where result[field.name] == nil {
            let candidates = [values[snippetID]?.exactValue(for: field.name), field.defaultValue]
            result[field.name] = candidates.compactMap { $0 }.first { value in
                field.kind != .choice || field.options.contains { $0.isExactly(value) }
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
