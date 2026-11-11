use super::{records, RecordKind, RemoteRecord, Session};
use objc2::AllocAnyThread;
use objc2_cloud_kit::CKRecord;
use objc2_foundation::NSString;

#[test]
fn unknown_send_results_do_not_acknowledge_or_fail_known_pending_records() {
    let good = RemoteRecord {
        kind: RecordKind::Meta,
        name: "meta".into(),
        payload: "{}".into(),
    };
    let session = Session::new(2, |_, _| {});
    session
        .state
        .lock()
        .unwrap()
        .pending
        .upsert(super::CloudBatch {
            records: vec![good.clone()],
            ..Default::default()
        });
    let unknown = unsafe {
        CKRecord::initWithRecordType_recordID(
            CKRecord::alloc(),
            &NSString::from_str("FutureKind"),
            &records::record_id("meta"),
        )
    };
    let report = crate::cloudkit::sending::apply_results(
        &session,
        crate::cloudkit::sending::SendResults {
            failed_saves: vec![(
                unknown,
                super::engine::error(objc2_cloud_kit::CKErrorCode::QuotaExceeded),
            )],
            ..Default::default()
        },
    );
    assert_eq!(report, super::CloudSend::default());
    assert_eq!(session.state.lock().unwrap().pending.saves(), vec![good]);
}

#[test]
fn unknown_types_and_unparseable_names_never_reach_fetched_batches() {
    let good = RemoteRecord {
        kind: RecordKind::Meta,
        name: "meta".into(),
        payload: "{\"syncVersion\":1}".into(),
    };
    let wrong_name = RemoteRecord {
        kind: RecordKind::Snippet,
        name: "snippet-not-a-uuid".into(),
        payload: "private contents must not be logged".into(),
    };
    let wrong_kind = RemoteRecord {
        kind: RecordKind::Group,
        name: "snippet-00000000-0000-0000-0000-000000000001".into(),
        payload: "{}".into(),
    };
    let unknown_type = unsafe {
        CKRecord::initWithRecordType_recordID(
            CKRecord::alloc(),
            &NSString::from_str("UnknownKind"),
            &records::record_id("meta"),
        )
    };
    let session = Session::new(8, |_, _| {});
    let batch = session.fetched(
        [
            records::build(&good, None),
            records::build(&wrong_name, None),
            records::build(&wrong_kind, None),
            unknown_type,
        ],
        [(
            records::record_id("unknown-deletion"),
            "KudzuSnippet".into(),
        )],
    );
    assert_eq!(batch.records, vec![good]);
    assert!(batch.deleted.is_empty());
}
