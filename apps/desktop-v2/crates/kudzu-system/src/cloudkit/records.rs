//! Only the frozen `payload` field crosses the wire; JSON is never parsed or rewritten.
use kudzu_core::sync::{RecordKind, RemoteRecord, META_RECORD, PAYLOAD_FIELD, ZONE_NAME};
use objc2::{
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    AllocAnyThread,
};
use objc2_cloud_kit::{CKCurrentUserDefaultName, CKRecord, CKRecordID, CKRecordZoneID};
use objc2_foundation::{NSCopying, NSString};

pub(super) fn zone_id() -> Retained<CKRecordZoneID> {
    unsafe {
        CKRecordZoneID::initWithZoneName_ownerName(
            CKRecordZoneID::alloc(),
            &NSString::from_str(ZONE_NAME),
            CKCurrentUserDefaultName,
        )
    }
}

pub(super) fn record_id(name: &str) -> Retained<CKRecordID> {
    unsafe {
        CKRecordID::initWithRecordName_zoneID(
            CKRecordID::alloc(),
            &NSString::from_str(name),
            &zone_id(),
        )
    }
}

pub(super) fn our_zone(id: &CKRecordZoneID) -> bool {
    unsafe { id.zoneName() }.to_string() == ZONE_NAME
}

pub(super) fn name(id: &CKRecordID) -> Option<String> {
    if !our_zone(&*unsafe { id.zoneID() }) {
        return None;
    }
    let name = unsafe { id.recordName() }.to_string();
    name_kind(&name)?;
    Some(name)
}

fn name_kind(name: &str) -> Option<RecordKind> {
    if name == META_RECORD {
        return Some(RecordKind::Meta);
    }
    for (prefix, kind) in [
        ("snippet-", RecordKind::Snippet),
        ("group-", RecordKind::Group),
    ] {
        if let Some(id) = name.strip_prefix(prefix) {
            uuid::Uuid::parse_str(id).ok()?;
            return Some(kind);
        }
    }
    name.strip_prefix("usage-")
        .filter(|device| !device.is_empty())
        .map(|_| RecordKind::Usage)
}

fn typed_name(id: &CKRecordID, record_type: &str) -> Option<(RecordKind, String)> {
    let name = name(id)?;
    let kind = RecordKind::from_record_type(record_type)?;
    (name_kind(&name) == Some(kind)).then_some((kind, name))
}

pub(super) fn identity(record: &CKRecord) -> Option<(RecordKind, String)> {
    typed_name(
        &*unsafe { record.recordID() },
        &unsafe { record.recordType() }.to_string(),
    )
}

pub(super) fn deletion_name(id: &CKRecordID, record_type: &str) -> Option<String> {
    typed_name(id, record_type).map(|(_, name)| name)
}

pub(super) fn remote(record: &CKRecord) -> Option<RemoteRecord> {
    let (kind, name) = identity(record)?;
    let value = unsafe { record.objectForKey(&NSString::from_str(PAYLOAD_FIELD)) }?;
    // CKRecordValue is an Objective-C object. Downcast before reading it as a string.
    let value: Retained<AnyObject> = unsafe { Retained::cast_unchecked(value) };
    let payload = value.downcast::<NSString>().ok()?.to_string();
    Some(RemoteRecord {
        name,
        kind,
        payload,
    })
}

/// The log sink accepts only a count, never a record, name, payload, or NSError.
pub(super) fn log_dropped(count: usize) {
    if count != 0 {
        log::warn!("sync: dropped {count} unsupported CloudKit records");
    }
}

pub(super) fn build(remote: &RemoteRecord, cached: Option<&CKRecord>) -> Retained<CKRecord> {
    // Copy the cached record to preserve its change tag without mutating an object
    // already held by an in-flight batch or another callback.
    let record = cached
        .map(|record| record.copy())
        .unwrap_or_else(|| unsafe {
            CKRecord::initWithRecordType_recordID(
                CKRecord::alloc(),
                &NSString::from_str(remote.kind.record_type()),
                &record_id(&remote.name),
            )
        });
    let payload = NSString::from_str(&remote.payload);
    unsafe {
        record.setObject_forKey(
            Some(ProtocolObject::from_ref(&*payload)),
            &NSString::from_str(PAYLOAD_FIELD),
        );
    }
    record
}
