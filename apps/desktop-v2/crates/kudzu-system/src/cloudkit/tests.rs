use super::{delivery::Delivery, errors, pending::Pending, records};
use kudzu_app::{
    error::PortError,
    ports::{CloudBatch, CloudSend, CloudSendFailure},
};
use kudzu_core::sync::{RecordKind, RemoteRecord};
use objc2_cloud_kit::CKErrorCode;
use std::sync::{Arc, Barrier, Mutex};

use super::{
    native::{Event, Session, State},
    sending,
};
use objc2::{
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    AllocAnyThread,
};
use objc2_cloud_kit::{
    CKCurrentUserDefaultName, CKErrorDomain, CKRecord, CKRecordChangedErrorServerRecordKey,
    CKRecordID, CKRecordZoneID, CKSyncEngineAccountChangeType, CKSyncEngineSendChangesScope,
};
use objc2_foundation::{NSDictionary, NSError, NSSet, NSString};

fn record(payload: &str) -> RemoteRecord {
    RemoteRecord {
        kind: RecordKind::Snippet,
        name: "snippet-test".into(),
        payload: payload.into(),
    }
}

#[test]
fn pending_upserts_are_idempotent_and_last_operation_wins() {
    let mut pending = Pending::default();
    let first = CloudBatch {
        records: vec![record("old")],
        ..Default::default()
    };
    pending.upsert(first.clone());
    pending.upsert(first);
    assert_eq!(pending.saves(), vec![record("old")]);
    pending.upsert(CloudBatch {
        records: vec![record("new")],
        ..Default::default()
    });
    assert_eq!(pending.saves(), vec![record("new")]);
    pending.upsert(CloudBatch {
        deleted: vec!["snippet-test".into()],
        ..Default::default()
    });
    assert!(pending.saves().is_empty());
    assert_eq!(pending.deletes(), vec!["snippet-test"]);
    pending.upsert(CloudBatch {
        records: vec![record("again")],
        ..Default::default()
    });
    pending.upsert(CloudBatch::default());
    assert!(pending.deletes().is_empty());
    assert_eq!(pending.saves(), vec![record("again")]);
}

#[test]
fn old_acknowledgments_do_not_erase_newer_pending_work() {
    let mut pending = Pending::default();
    pending.upsert(CloudBatch {
        records: vec![record("new")],
        ..Default::default()
    });
    pending.saved(&record("old"));
    pending.deleted("snippet-test");
    assert_eq!(pending.saves(), vec![record("new")]);
    pending.saved(&record("new"));
    assert!(pending.is_empty());
    pending.upsert(CloudBatch {
        deleted: vec!["snippet-test".into()],
        ..Default::default()
    });
    pending.saved(&record("new"));
    assert_eq!(pending.deletes(), vec!["snippet-test"]);
    pending.deleted("snippet-test");
    assert!(pending.is_empty());
}

#[test]
fn transient_failures_never_enter_the_send_report() {
    for code in [
        CKErrorCode::NetworkUnavailable,
        CKErrorCode::NetworkFailure,
        CKErrorCode::ServiceUnavailable,
        CKErrorCode::RequestRateLimited,
        CKErrorCode::ZoneBusy,
    ] {
        let mut report = CloudSend::default();
        assert!(!errors::send_failure(&mut report, code));
        assert_eq!(report, CloudSend::default());
    }
    let mut report = CloudSend::default();
    assert!(errors::send_failure(
        &mut report,
        CKErrorCode::QuotaExceeded
    ));
    assert_eq!(report.failure, Some(CloudSendFailure::QuotaExceeded));
    errors::send_failure(&mut report, CKErrorCode::InvalidArguments);
    assert_eq!(report.failure, Some(CloudSendFailure::QuotaExceeded));
    let mut report = CloudSend::default();
    errors::send_failure(&mut report, CKErrorCode(9999));
    assert_eq!(report.failure, Some(CloudSendFailure::Permanent));
}

#[test]
fn each_session_keeps_its_generation_and_stops_delivering() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let output = received.clone();
    let old = Delivery::new(7, move |generation, value| {
        output.lock().unwrap().push((generation, value))
    });
    old.emit("before");
    old.stop();
    old.emit("stale");
    let output = received.clone();
    let new = Delivery::new(8, move |generation, value| {
        output.lock().unwrap().push((generation, value))
    });
    new.emit("after");
    assert_eq!(*received.lock().unwrap(), vec![(7, "before"), (8, "after")]);
}

#[test]
fn stop_waits_for_admitted_callbacks_without_holding_the_delivery_lock() {
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let entered_callback = entered.clone();
    let release_callback = release.clone();
    let delivery = Arc::new(Delivery::new(3, move |_, ()| {
        entered_callback.wait();
        release_callback.wait();
    }));
    std::thread::scope(|scope| {
        let callback = scope.spawn(|| delivery.emit(()));
        entered.wait();
        let stop = scope.spawn(|| delivery.stop());
        while delivery.is_active() {
            std::thread::yield_now();
        }
        assert!(delivery.enter().is_none());
        release.wait();
        callback.join().unwrap();
        stop.join().unwrap();
    });
    delivery.emit(());
}

#[test]
fn unentitled_start_never_enters_the_cloudkit_factory() {
    assert_eq!(
        super::entitlement::require(false, || -> Result<(), PortError> {
            panic!("CloudKit must not be touched");
        }),
        Err(PortError::Unavailable)
    );
}

#[test]
fn payload_translation_preserves_bytes_and_all_four_record_types() {
    let payload = " {\"text\":\"مرحبا \\n café\",\"future\":{\"z\":1}} ";
    for kind in [
        RecordKind::Snippet,
        RecordKind::Group,
        RecordKind::Usage,
        RecordKind::Meta,
    ] {
        let original = RemoteRecord {
            kind,
            name: "unchanged-record-name".into(),
            payload: payload.into(),
        };
        let native = records::build(&original, None);
        assert_eq!(records::remote(&native), Some(original));
    }
}

fn error(code: CKErrorCode, server: Option<&CKRecord>) -> Retained<NSError> {
    let info = server.map(|server| {
        NSDictionary::<NSString, AnyObject>::from_slices(
            &[unsafe { CKRecordChangedErrorServerRecordKey }],
            &[server.as_ref()],
        )
    });
    unsafe { NSError::errorWithDomain_code_userInfo(CKErrorDomain, code.0, info.as_deref()) }
}

#[test]
fn conflict_replaces_the_cached_record_before_requeue_and_waits_for_the_service() {
    let original = record("old");
    let winner = record("server");
    let server = records::build(&winner, None);
    let mut state = State::default();
    state.pending.upsert(CloudBatch {
        records: vec![original.clone()],
        ..Default::default()
    });
    state
        .records
        .insert(original.name.clone(), records::build(&original, None));
    let mut report = CloudSend::default();
    sending::failed(
        &mut state,
        &mut report,
        original.name.clone(),
        &error(CKErrorCode::ServerRecordChanged, Some(&server)),
    );
    assert_eq!(report.conflicts, vec![winner.clone()]);
    assert_eq!(report.requeue, vec!["snippet-test"]);
    assert_eq!(
        records::remote(&state.records["snippet-test"]),
        Some(winner)
    );
    assert_ne!(
        Retained::as_ptr(&state.records["snippet-test"]),
        Retained::as_ptr(&server)
    );
    assert!(state.pending.is_empty());
    assert_eq!(report.failure, None);
}

#[test]
fn unknown_item_and_missing_zone_drop_the_cache_without_inventing_conflicts() {
    for code in [CKErrorCode::UnknownItem, CKErrorCode::ZoneNotFound] {
        let mut state = State::default();
        state.records.insert(
            "snippet-test".into(),
            records::build(&record("cached"), None),
        );
        state.pending.upsert(CloudBatch {
            records: vec![record("pending")],
            ..Default::default()
        });
        let mut report = CloudSend::default();
        sending::failed(
            &mut state,
            &mut report,
            "snippet-test".into(),
            &error(code, None),
        );
        assert!(state.records.is_empty());
        assert!(state.pending.is_empty());
        assert!(report.conflicts.is_empty());
        assert_eq!(report.requeue, vec!["snippet-test"]);
        assert_eq!(report.zone_missing, code == CKErrorCode::ZoneNotFound);
        assert_eq!(report.failure, None);
    }
}

#[test]
fn retryable_send_failure_keeps_pending_but_permanent_failure_does_not() {
    let mut state = State::default();
    state.pending.upsert(CloudBatch {
        records: vec![record("unsent")],
        ..Default::default()
    });
    let mut report = CloudSend::default();
    sending::failed(
        &mut state,
        &mut report,
        "snippet-test".into(),
        &error(CKErrorCode::NetworkFailure, None),
    );
    assert_eq!(state.pending.saves(), vec![record("unsent")]);
    assert_eq!(report, CloudSend::default());
    sending::failed(
        &mut state,
        &mut report,
        "snippet-test".into(),
        &error(CKErrorCode::QuotaExceeded, None),
    );
    assert!(state.pending.is_empty());
    assert_eq!(report.failure, Some(CloudSendFailure::QuotaExceeded));
}

fn foreign_id(name: &str) -> Retained<CKRecordID> {
    unsafe {
        let zone = CKRecordZoneID::initWithZoneName_ownerName(
            CKRecordZoneID::alloc(),
            &NSString::from_str("SwiftData"),
            CKCurrentUserDefaultName,
        );
        CKRecordID::initWithRecordName_zoneID(CKRecordID::alloc(), &NSString::from_str(name), &zone)
    }
}

#[test]
fn fetched_changes_only_report_our_zone_and_well_typed_payloads() {
    let session = Session::new(5, |_, _| {});
    let foreign = unsafe {
        CKRecord::initWithRecordType_recordID(
            CKRecord::alloc(),
            &NSString::from_str("KudzuSnippet"),
            &foreign_id("snippet-foreign"),
        )
    };
    let missing_payload = unsafe {
        CKRecord::initWithRecordType_recordID(
            CKRecord::alloc(),
            &NSString::from_str("KudzuSnippet"),
            &records::record_id("snippet-empty"),
        )
    };
    let unknown_type = unsafe {
        CKRecord::initWithRecordType_recordID(
            CKRecord::alloc(),
            &NSString::from_str("FutureRecord"),
            &records::record_id("future-record"),
        )
    };
    let ours = records::build(&record("remote"), None);
    let batch = session.fetched(
        vec![foreign, missing_payload, unknown_type, ours],
        vec![foreign_id("snippet-deleted")],
    );
    assert_eq!(batch.records, vec![record("remote")]);
    assert!(batch.deleted.is_empty());
    assert!(!batch.full_fetch);
    assert_eq!(session.state.lock().unwrap().records.len(), 1);
    let deletion = session.fetched([], [records::record_id("snippet-test")]);
    assert_eq!(deletion.deleted, vec!["snippet-test"]);
    assert!(session.state.lock().unwrap().records.is_empty());
}

#[test]
fn record_updates_copy_the_server_record_and_keep_unknown_fields() {
    let server = records::build(&record("before"), None);
    let marker = NSString::from_str("preserved");
    unsafe {
        server.setObject_forKey(
            Some(ProtocolObject::from_ref(&*marker)),
            &NSString::from_str("future"),
        );
    }
    let updated = records::build(&record("after"), Some(&server));
    assert_eq!(records::remote(&server), Some(record("before")));
    assert_eq!(records::remote(&updated), Some(record("after")));
    assert!(unsafe { updated.objectForKey(&NSString::from_str("future")) }.is_some());
    assert_ne!(Retained::as_ptr(&updated), Retained::as_ptr(&server));
}

#[test]
fn zone_recreation_precedes_notification_and_clears_transport_knowledge() {
    let actions = Mutex::new(Vec::new());
    let session = Session::new(11, |_, _| {});
    session.state.lock().unwrap().pending.upsert(CloudBatch {
        records: vec![record("pending")],
        ..Default::default()
    });
    let event = session.zone_deleted(|| actions.lock().unwrap().push("recreated"));
    assert!(matches!(event, Event::ZoneDeleted));
    actions.lock().unwrap().push("notified");
    assert_eq!(*actions.lock().unwrap(), vec!["recreated", "notified"]);
    assert!(session.state.lock().unwrap().pending.is_empty());
}

#[test]
fn every_account_change_refreshes_availability_but_sign_in_keeps_pending_work() {
    for change in [
        CKSyncEngineAccountChangeType::SignIn,
        CKSyncEngineAccountChangeType::SignOut,
        CKSyncEngineAccountChangeType::SwitchAccounts,
    ] {
        let session = Session::new(9, |_, _| {});
        session.state.lock().unwrap().pending.upsert(CloudBatch {
            records: vec![record("pending")],
            ..Default::default()
        });
        let mut refreshed = false;
        let event = session.account_changed(change, || refreshed = true);
        assert!(refreshed);
        assert!(matches!(event, Event::Account(actual) if actual == change));
        assert_eq!(
            session.state.lock().unwrap().pending.is_empty(),
            change != CKSyncEngineAccountChangeType::SignIn
        );
    }
}

#[test]
fn native_batches_only_include_admitted_names_and_honor_the_scope() {
    let session = Session::new(2, |_, _| {});
    let scope = unsafe {
        CKSyncEngineSendChangesScope::initWithZoneIDs(
            CKSyncEngineSendChangesScope::alloc(),
            Some(&NSSet::from_retained_slice(&[records::zone_id()])),
        )
    };
    let names = ["snippet-test".into(), "old-restored-deletion".into()]
        .into_iter()
        .collect();
    assert!(sending::build_batch(&session, &scope, &names).is_none());
    session.state.lock().unwrap().pending.upsert(CloudBatch {
        records: vec![record("latest")],
        ..Default::default()
    });
    let batch = sending::build_batch(&session, &scope, &names).unwrap();
    assert_eq!(
        unsafe { batch.recordsToSave() }
            .iter()
            .filter_map(|r| records::remote(&r))
            .collect::<Vec<_>>(),
        vec![record("latest")]
    );
    assert!(unsafe { batch.recordIDsToDelete() }.is_empty());
    let foreign_scope = unsafe {
        CKSyncEngineSendChangesScope::initWithZoneIDs(
            CKSyncEngineSendChangesScope::alloc(),
            Some(&NSSet::from_retained_slice(&[foreign_id("x").zoneID()])),
        )
    };
    assert!(sending::build_batch(&session, &foreign_scope, &names).is_none());
}

#[test]
fn state_updates_keep_the_delegate_order_and_stop_drops_all_event_kinds() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let output = received.clone();
    let session = Session::new(15, move |generation, event| {
        let label = match event {
            Event::Fetched(_) => "fetch",
            Event::State(_) => "state",
            Event::Sent(_) => "send",
            Event::Account(_) => "account",
            Event::Availability(_) => "availability",
            Event::Failed(_) => "failed",
            Event::ZoneDeleted => "zone",
        };
        output.lock().unwrap().push((generation, label));
    });
    session.delivery.emit(Event::Fetched(CloudBatch::default()));
    session.delivery.emit(Event::State(vec![1]));
    session.delivery.emit(Event::Sent(CloudSend::default()));
    session.delivery.emit(Event::State(vec![2]));
    session.delivery.stop();
    for event in [
        Event::Fetched(CloudBatch::default()),
        Event::Sent(CloudSend::default()),
        Event::State(vec![3]),
        Event::Account(CKSyncEngineAccountChangeType::SignOut),
        Event::Availability(objc2_cloud_kit::CKAccountStatus::NoAccount),
        Event::Failed(PortError::Unavailable),
        Event::ZoneDeleted,
    ] {
        session.delivery.emit(event);
    }
    assert_eq!(
        *received.lock().unwrap(),
        vec![(15, "fetch"), (15, "state"), (15, "send"), (15, "state")]
    );
}
