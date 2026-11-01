import Foundation
import SwiftData

/// Usage counter for one (snippet, device) pair. Summed across devices for display.
///
/// One row per device because CloudKit resolves conflicts last-writer-wins:
/// a shared counter incremented on two Macs would lose increments, while
/// per-device rows are only ever written by their own device.
///
/// `snippetID` is a plain attribute, not a relationship, on purpose:
/// - A relationship with nullify would orphan (and with cascade, delete) the
///   stats whenever dedupe deletes a merged snippet; a UUID can be re-pointed.
/// - Rows sync independently of snippets; a row may arrive before its snippet
///   (or after it was merged away) without dangling-reference handling.
/// - Recording a use doesn't fault in or dirty the snippet record, so an
///   expansion only exports one small usage record to CloudKit.
@Model
public final class UsageModel {
    /// Row identity. Usage records have no ID in the interchange format; this one is local to sync.
    public var id: UUID = UUID()
    public var snippetID: UUID = UUID()
    public var deviceID: String = ""
    public var count: Int = 0
    public var charsSaved: Int = 0
    public var lastUsedAt: Date?

    /// Creates a model from a record.
    public init(_ record: UsageRecord) {
        update(from: record)
    }

    /// Value snapshot of this row.
    public var record: UsageRecord {
        UsageRecord(snippetID: snippetID, deviceID: deviceID, count: count, charsSaved: charsSaved, lastUsedAt: lastUsedAt)
    }

    /// Copies every field from `record`, skipping unchanged values.
    public func update(from record: UsageRecord) {
        if snippetID != record.snippetID { snippetID = record.snippetID }
        if deviceID != record.deviceID { deviceID = record.deviceID }
        if count != record.count { count = record.count }
        if charsSaved != record.charsSaved { charsSaved = record.charsSaved }
        if lastUsedAt != record.lastUsedAt { lastUsedAt = record.lastUsedAt }
    }
}
