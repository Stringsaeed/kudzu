import Foundation
import SwiftData

/// Persistent snippet. Synced through CloudKit; convert to `SnippetRecord` for everything else.
///
/// CloudKit rules: every attribute is defaulted or optional, the relationship
/// is optional, and `id` is NOT unique-constrained (dedupe handles copies).
/// Enums are stored as raw strings and read through typed accessors that fall
/// back to defaults, so values from newer clients never crash older ones.
@Model
public final class SnippetModel {
    /// Stable identity, equal to `SnippetRecord.id`. Not unique-constrained.
    public var id: UUID = UUID()
    public var name: String = ""
    /// Storage for `triggers`: JSON array of strings (see `StoredValues`).
    public var triggersJSON: String = "[]"
    /// Template source; see docs/TEMPLATE_SYNTAX.md.
    public var content: String = ""
    /// Storage for `fireMode`.
    public var fireModeRaw: String = "immediate"
    /// Storage for `caseMode`.
    public var caseModeRaw: String = "propagate"
    /// Storage for `insertion`; nil inherits.
    public var insertionRaw: String?
    /// Storage for `rule`: JSON-encoded `AppRule`; nil inherits.
    public var ruleJSON: String?
    public var isFavorite: Bool = false
    public var isEnabled: Bool = true
    public var createdAt: Date = Date.distantPast
    public var updatedAt: Date = Date.distantPast
    /// IDs of snippets merged into this one by dedupe (JSON array of UUID strings).
    /// Lets usage rows that sync in later, still pointing at a merged-away
    /// snippet, find their survivor.
    public var mergedIDsJSON: String = "[]"

    @Relationship(deleteRule: .nullify, inverse: \GroupModel.snippets)
    public var group: GroupModel?

    /// Creates a model from a record. Doesn't set `group`; the store resolves it.
    public init(_ record: SnippetRecord) {
        id = record.id
        update(from: record)
    }

    /// Triggers, decoded from `triggersJSON`.
    public var triggers: [String] {
        get { StoredValues.decodeStrings(triggersJSON) }
        set { triggersJSON = StoredValues.encodeStrings(newValue) }
    }

    /// Fire mode; unknown stored values read as `.immediate`.
    public var fireMode: FireMode {
        get { FireMode(rawValue: fireModeRaw) ?? .immediate }
        set { fireModeRaw = newValue.rawValue }
    }

    /// Case mode; unknown stored values read as `.propagate`.
    public var caseMode: CaseMode {
        get { CaseMode(rawValue: caseModeRaw) ?? .propagate }
        set { caseModeRaw = newValue.rawValue }
    }

    /// Insertion override; unknown stored values read as nil (inherit).
    public var insertion: InsertionMethod? {
        get { insertionRaw.decodedEnum(InsertionMethod.self) }
        set { insertionRaw = newValue?.rawValue }
    }

    /// App rule; undecodable stored values read as nil (inherit).
    public var rule: AppRule? {
        get { StoredValues.decodeRule(ruleJSON) }
        set { ruleJSON = StoredValues.encodeRule(newValue) }
    }

    /// Snippet IDs previously merged into this one.
    public var mergedIDs: [UUID] {
        get { StoredValues.decodeUUIDs(mergedIDsJSON) }
        set { mergedIDsJSON = StoredValues.encodeUUIDs(newValue) }
    }

    /// Value snapshot of this snippet.
    public var record: SnippetRecord {
        SnippetRecord(
            id: id,
            name: name,
            triggers: triggers,
            content: content,
            groupID: group?.id,
            fireMode: fireMode,
            caseMode: caseMode,
            insertion: insertion,
            rule: rule,
            isFavorite: isFavorite,
            isEnabled: isEnabled,
            createdAt: createdAt,
            updatedAt: updatedAt
        )
    }

    /// Copies every field except `id` and `group` from `record`.
    ///
    /// Only assigns changed values: every write marks the record dirty and
    /// triggers a CloudKit export, so no-op updates should stay no-ops.
    public func update(from record: SnippetRecord) {
        assign(\.name, record.name)
        assign(\.triggersJSON, StoredValues.encodeStrings(record.triggers))
        assign(\.content, record.content)
        assign(\.fireModeRaw, record.fireMode.rawValue)
        assign(\.caseModeRaw, record.caseMode.rawValue)
        assign(\.insertionRaw, record.insertion?.rawValue)
        // Compare decoded rules so an equivalent rule stored by an older encoder isn't rewritten.
        if rule != record.rule { ruleJSON = StoredValues.encodeRule(record.rule) }
        assign(\.isFavorite, record.isFavorite)
        assign(\.isEnabled, record.isEnabled)
        assign(\.createdAt, record.createdAt)
        assign(\.updatedAt, record.updatedAt)
    }

    private func assign<T: Equatable>(_ keyPath: ReferenceWritableKeyPath<SnippetModel, T>, _ value: T) {
        if self[keyPath: keyPath] != value { self[keyPath: keyPath] = value }
    }
}
