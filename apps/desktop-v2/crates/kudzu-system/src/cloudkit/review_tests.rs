mod engine;
mod fetching;
mod pool;
mod sending;
mod translation;

use super::{
    native::{Event, Session},
    records, requests,
};
use kudzu_app::ports::{CloudBatch, CloudSend};
use kudzu_core::sync::{RecordKind, RemoteRecord, PAYLOAD_FIELD, ZONE_NAME};
use objc2::rc::Retained;
use objc2::{runtime::ProtocolObject, AllocAnyThread};
use objc2_cloud_kit::{CKRecord, CKRecordID, CKRecordZoneID};
use objc2_foundation::NSString;

#[test]
fn empty_state_archives_are_dropped_without_failure() {
    assert!(super::delegate::state_event(Ok(Vec::new())).is_none());
    assert!(
        matches!(super::delegate::state_event(Ok(vec![0, 1])), Some(Event::State(bytes)) if bytes == [0, 1])
    );
}
use std::sync::{Arc, Mutex};

#[test]
fn restored_pending_records_are_removed_before_availability() {
    use super::engine::SyncEngine;
    use objc2_cloud_kit::{
        CKSyncEnginePendingRecordZoneChange as Change,
        CKSyncEnginePendingRecordZoneChangeType as Kind,
    };
    let engine = engine::Engine::default();
    let restored = unsafe {
        [
            Change::initWithRecordID_type(
                Change::alloc(),
                &records::record_id("meta"),
                Kind::SaveRecord,
            ),
            Change::initWithRecordID_type(
                Change::alloc(),
                &records::record_id("obsolete-restored-name"),
                Kind::DeleteRecord,
            ),
        ]
    };
    engine.add(&restored);
    requests::start(&engine, false, || {
        assert!(
            engine.pending().is_empty(),
            "restored IDs have no current service-owned payloads"
        )
    });
    assert_eq!(
        engine.zones.get(),
        0,
        "a restored database operation is not a first-launch zone save"
    );
}

#[test]
fn send_completion_does_not_overwrite_a_delegate_conflict() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let out = received.clone();
    let session = Arc::new(Session::new(5, move |_, event| {
        out.lock().unwrap().push(event)
    }));
    let delegate = session.clone();
    let engine = engine::Engine {
        on_send: Some(Box::new(move |completion| {
            delegate.delivery.emit(Event::Sent(CloudSend {
                requeue: vec!["meta".into()],
                ..Default::default()
            }));
            if let Some(completion) = completion {
                let error = engine::error(objc2_cloud_kit::CKErrorCode::PartialFailure);
                completion.call((Retained::as_ptr(&error).cast_mut(),));
            }
        })),
        ..Default::default()
    };
    requests::send(&engine, &session, CloudBatch::default());
    let events = received.lock().unwrap();
    assert_eq!(
        events.len(),
        1,
        "the completion must not add an empty Permanent report"
    );
    assert!(
        matches!(&events[0], Event::Sent(report) if report.requeue == ["meta"] && report.failure.is_none())
    );
}

#[test]
fn missing_zone_fetch_recreates_without_reporting_failure() {
    let session = Session::new(1, |_, _| {});
    let mut recreated = false;
    let result = super::delegate::zone_fetch_result(
        &session,
        Some(objc2_cloud_kit::CKErrorCode::ZoneNotFound),
        || recreated = true,
    );
    assert!(
        result.is_none(),
        "a first-launch zone race is not a permanent fetch failure"
    );
    assert!(recreated);
}

#[test]
fn transient_fetch_keeps_delivered_changes_but_has_no_terminal_result() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let out = received.clone();
    let session = Arc::new(Session::new(12, move |_, event| {
        out.lock().unwrap().push(event)
    }));
    let delegate = session.clone();
    let engine = engine::Engine {
        on_fetch: Some(Box::new(move |completion| {
            let record = RemoteRecord {
                kind: RecordKind::Meta,
                name: "meta".into(),
                payload: "{\"syncVersion\":1}".into(),
            };
            delegate.delivery.emit(Event::Fetched(
                delegate.fetched([records::build(&record, None)], []),
            ));
            delegate.delivery.emit(Event::State(vec![1, 2, 3]));
            if let Some(event) = super::delegate::zone_fetch_result(
                &delegate,
                Some(objc2_cloud_kit::CKErrorCode::NetworkFailure),
                || {},
            ) {
                delegate.delivery.emit(event);
            }
            if let Some(event) = super::delegate::did_fetch(&delegate) {
                delegate.delivery.emit(event);
            }
            let error = engine::error(objc2_cloud_kit::CKErrorCode::NetworkFailure);
            completion
                .unwrap()
                .call((Retained::as_ptr(&error).cast_mut(),));
        })),
        ..Default::default()
    };
    requests::fetch(&engine, &session);
    let events = received.lock().unwrap();
    assert_eq!(
        events.len(),
        2,
        "transient failure must not add a terminal batch or failure"
    );
    assert!(
        matches!(&events[0], Event::Fetched(batch) if !batch.full_fetch && batch.records.len() == 1)
    );
    assert!(matches!(&events[1], Event::State(bytes) if bytes == &[1,2,3]));
}

#[test]
fn kudzu_zone_records_accept_a_concrete_user_owner() {
    let (zone, id, record) = unsafe {
        let zone = CKRecordZoneID::initWithZoneName_ownerName(
            CKRecordZoneID::alloc(),
            &NSString::from_str(ZONE_NAME),
            &NSString::from_str("_concrete-user-record-id"),
        );
        let id = CKRecordID::initWithRecordName_zoneID(
            CKRecordID::alloc(),
            &NSString::from_str("meta"),
            &zone,
        );
        let record = CKRecord::initWithRecordType_recordID(
            CKRecord::alloc(),
            &NSString::from_str("KudzuMeta"),
            &id,
        );
        let payload = NSString::from_str("{\"syncVersion\":1}");
        record.setObject_forKey(
            Some(ProtocolObject::from_ref(&*payload)),
            &NSString::from_str(PAYLOAD_FIELD),
        );
        (zone, id, record)
    };
    assert!(
        records::our_zone(&zone),
        "zone deletions must recognize this owner too"
    );
    assert_eq!(records::name(&id).as_deref(), Some("meta"));
    let expected = RemoteRecord {
        kind: RecordKind::Meta,
        name: "meta".into(),
        payload: "{\"syncVersion\":1}".into(),
    };
    assert_eq!(
        records::remote(&record),
        Some(expected.clone()),
        "send acknowledgments use the same translation"
    );
    let session = Session::new(7, |_, _| {});
    assert_eq!(session.fetched([record], []).records, vec![expected]);
}
