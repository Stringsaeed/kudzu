import Foundation
import KudzuCore

/// Converts between Core values and persisted rows. Reading never substitutes a default for a
/// payload that does not decode: the error names the row and the field. Writing assigns only
/// attributes whose value changed, so unchanged rows produce no unnecessary persisted changes
/// or mirroring work. Comparing through a key path avoids SwiftData's inout writeback.
enum LibraryRowCodec {
    private static let encoder = KudzuJSON.encoder(pretty: false)
    private static let decoder = KudzuJSON.decoder()

    // MARK: Snippets

    static func snippet(from row: PersistedSnippet) throws -> Snippet {
        let id = row.id.lowercased
        return Snippet(
            id: row.id,
            name: try text(row.nameUTF8, entity: "PersistedSnippet", id: id, field: "nameUTF8"),
            triggers: try decode([String].self, row.triggersJSON, "PersistedSnippet", id, "triggersJSON"),
            content: try text(row.contentUTF8, entity: "PersistedSnippet", id: id, field: "contentUTF8"),
            formatting: try row.formattingJSON.map { try decode(Formatting.self, $0, "PersistedSnippet", id, "formattingJSON") },
            groupID: row.groupID,
            fireMode: try rawValue(FireMode.self, row.fireModeRaw, "PersistedSnippet", id, "fireModeRaw"),
            caseMode: try rawValue(CaseMode.self, row.caseModeRaw, "PersistedSnippet", id, "caseModeRaw"),
            insertion: try row.insertionRaw.map { try rawValue(InsertionMethod.self, $0, "PersistedSnippet", id, "insertionRaw") },
            rule: try row.ruleJSON.map { try decode(AppRule.self, $0, "PersistedSnippet", id, "ruleJSON") },
            isFavorite: row.isFavorite,
            isEnabled: row.isEnabled,
            createdAt: row.createdAt,
            updatedAt: row.updatedAt
        )
    }

    static func apply(_ snippet: Snippet, listOrder: Int, to row: PersistedSnippet, baseline: Snippet? = nil, baselineOrder: Int? = nil) throws {
        assign(row, \.id, snippet.id, previous: baseline.map { $0.id })
        assign(row, \.nameUTF8, Data(snippet.name.utf8), previous: baseline.map { Data($0.name.utf8) })
        assign(row, \.triggersJSON, try encoder.encode(snippet.triggers), previous: try baseline.map { try encoder.encode($0.triggers) })
        assign(row, \.contentUTF8, Data(snippet.content.utf8), previous: baseline.map { Data($0.content.utf8) })
        assign(row, \.formattingJSON, try snippet.formatting.map { try encoder.encode($0) }, previous: try baseline.map { try $0.formatting.map { try encoder.encode($0) } })
        assign(row, \.groupID, snippet.groupID, previous: baseline.map { $0.groupID })
        assign(row, \.fireModeRaw, snippet.fireMode.rawValue, previous: baseline.map { $0.fireMode.rawValue })
        assign(row, \.caseModeRaw, snippet.caseMode.rawValue, previous: baseline.map { $0.caseMode.rawValue })
        assign(row, \.insertionRaw, snippet.insertion?.rawValue, previous: baseline.map { $0.insertion?.rawValue })
        assign(row, \.ruleJSON, try snippet.rule.map { try encoder.encode($0) }, previous: try baseline.map { try $0.rule.map { try encoder.encode($0) } })
        assign(row, \.isFavorite, snippet.isFavorite, previous: baseline.map { $0.isFavorite })
        assign(row, \.isEnabled, snippet.isEnabled, previous: baseline.map { $0.isEnabled })
        assign(row, \.createdAt, snippet.createdAt, previous: baseline.map { $0.createdAt })
        assign(row, \.updatedAt, snippet.updatedAt, previous: baseline.map { $0.updatedAt })
        assign(row, \.listOrder, listOrder, previous: baselineOrder)
    }

    // MARK: Groups

    static func group(from row: PersistedGroup) throws -> Group {
        let id = row.id.lowercased
        return Group(
            id: row.id,
            name: try text(row.nameUTF8, entity: "PersistedGroup", id: id, field: "nameUTF8"),
            parentID: row.parentID,
            sortIndex: row.sortIndex,
            isEnabled: row.isEnabled,
            insertion: try row.insertionRaw.map { try rawValue(InsertionMethod.self, $0, "PersistedGroup", id, "insertionRaw") },
            rule: try row.ruleJSON.map { try decode(AppRule.self, $0, "PersistedGroup", id, "ruleJSON") }
        )
    }

    static func apply(_ group: Group, listOrder: Int, to row: PersistedGroup, baseline: Group? = nil, baselineOrder: Int? = nil) throws {
        assign(row, \.id, group.id, previous: baseline.map { $0.id })
        assign(row, \.nameUTF8, Data(group.name.utf8), previous: baseline.map { Data($0.name.utf8) })
        assign(row, \.parentID, group.parentID, previous: baseline.map { $0.parentID })
        assign(row, \.sortIndex, group.sortIndex, previous: baseline.map { $0.sortIndex })
        assign(row, \.isEnabled, group.isEnabled, previous: baseline.map { $0.isEnabled })
        assign(row, \.insertionRaw, group.insertion?.rawValue, previous: baseline.map { $0.insertion?.rawValue })
        assign(row, \.ruleJSON, try group.rule.map { try encoder.encode($0) }, previous: try baseline.map { try $0.rule.map { try encoder.encode($0) } })
        assign(row, \.listOrder, listOrder, previous: baselineOrder)
    }

    // MARK: Usage

    static func usage(from row: PersistedUsage) throws -> UsageRecord {
        UsageRecord(snippetID: row.snippetID, deviceID: try text(row.deviceIDUTF8, entity: "PersistedUsage", id: row.snippetID.lowercased, field: "deviceIDUTF8"), count: row.count, charsSaved: row.charsSaved, lastUsedAt: row.lastUsedAt)
    }

    static func apply(_ usage: UsageRecord, listOrder: Int, to row: PersistedUsage, baseline: UsageRecord? = nil, baselineOrder: Int? = nil) {
        assign(row, \.snippetID, usage.snippetID, previous: baseline.map { $0.snippetID })
        assign(row, \.deviceIDUTF8, Data(usage.deviceID.utf8), previous: baseline.map { Data($0.deviceID.utf8) })
        assign(row, \.count, usage.count, previous: baseline.map { $0.count })
        assign(row, \.charsSaved, usage.charsSaved, previous: baseline.map { $0.charsSaved })
        assign(row, \.lastUsedAt, usage.lastUsedAt, previous: baseline.map { $0.lastUsedAt })
        assign(row, \.listOrder, listOrder, previous: baselineOrder)
    }

    /// The lookup key of a usage row: one row per snippet per device.
    static func usageKey(snippetID: UUID, deviceID: String) -> String {
        snippetID.lowercased + "|" + Data(deviceID.utf8).base64EncodedString()
    }

    // Core Data string attributes can truncate embedded NULs. Binary UTF-8 also keeps
    // canonically equivalent spellings distinct during updates.
    private static func text(_ data: Data, entity: String, id: String, field: String) throws -> String {
        guard let value = String(data: data, encoding: .utf8) else {
            throw LibraryRepositoryError.unreadableField(entity: entity, id: id, field: field, reason: "invalid UTF-8")
        }
        return value
    }

    // MARK: Helpers

    private static func assign<Row: AnyObject, Value: Equatable>(
        _ row: Row, _ keyPath: ReferenceWritableKeyPath<Row, Value>, _ value: Value, previous: Value?
    ) {
        if let previous, previous == value { return }
        if row[keyPath: keyPath] != value { row[keyPath: keyPath] = value }
    }

    private static func decode<T: Decodable>(_ type: T.Type, _ data: Data, _ entity: String, _ id: String, _ field: String) throws -> T {
        do {
            return try decoder.decode(type, from: data)
        } catch {
            throw LibraryRepositoryError.unreadableField(entity: entity, id: id, field: field, reason: String(describing: error))
        }
    }

    private static func rawValue<T: RawRepresentable>(_ type: T.Type, _ raw: String, _ entity: String, _ id: String, _ field: String) throws -> T where T.RawValue == String {
        guard let value = T(rawValue: raw) else {
            throw LibraryRepositoryError.unreadableField(entity: entity, id: id, field: field, reason: "unknown value \"\(raw)\"")
        }
        return value
    }
}
