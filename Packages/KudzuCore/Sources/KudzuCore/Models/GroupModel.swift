import Foundation
import SwiftData

/// Persistent snippet group; groups nest through an optional self-referencing parent.
///
/// Children are unordered (CloudKit can't sync ordered relationships); order by `sortIndex`.
@Model
public final class GroupModel {
    /// Stable identity, equal to `GroupRecord.id`. Not unique-constrained.
    public var id: UUID = UUID()
    public var name: String = ""
    public var sortIndex: Int = 0
    public var isEnabled: Bool = true
    /// Storage for `insertion`; nil inherits.
    public var insertionRaw: String?
    /// Storage for `rule`: JSON-encoded `AppRule`; nil inherits.
    public var ruleJSON: String?

    public var parent: GroupModel?

    /// Deleting a group detaches (doesn't delete) its children; `KudzuStore.deleteGroup` re-parents them first.
    @Relationship(deleteRule: .nullify, inverse: \GroupModel.parent)
    public var children: [GroupModel]? = []

    public var snippets: [SnippetModel]? = []

    /// Creates a model from a record. Doesn't set `parent`; the store resolves it.
    public init(_ record: GroupRecord) {
        id = record.id
        update(from: record)
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

    /// Value snapshot of this group.
    public var record: GroupRecord {
        GroupRecord(
            id: id,
            name: name,
            parentID: parent?.id,
            sortIndex: sortIndex,
            isEnabled: isEnabled,
            insertion: insertion,
            rule: rule
        )
    }

    /// Copies every field except `id` and `parent` from `record`, skipping unchanged values.
    public func update(from record: GroupRecord) {
        if name != record.name { name = record.name }
        if sortIndex != record.sortIndex { sortIndex = record.sortIndex }
        if isEnabled != record.isEnabled { isEnabled = record.isEnabled }
        if insertionRaw != record.insertion?.rawValue { insertionRaw = record.insertion?.rawValue }
        if rule != record.rule { ruleJSON = StoredValues.encodeRule(record.rule) }
    }
}
