use super::{engine, records, CloudBatch, CloudSend, RecordKind, RemoteRecord, Session};
use crate::cloudkit::{native::State, sending};
use kudzu_app::ports::CloudSendFailure;
use objc2_cloud_kit::CKErrorCode;

fn record(id: u128, payload: &str) -> RemoteRecord {
    RemoteRecord {
        kind: RecordKind::Snippet,
        name: kudzu_core::sync::snippet_name(uuid::Uuid::from_u128(id)),
        payload: payload.into(),
    }
}

#[test]
fn a_conflict_without_a_server_copy_is_permanent_and_discards_the_old_tag() {
    let local = record(1, "local");
    let mut state = State::default();
    state
        .records
        .insert(local.name.clone(), records::build(&local, None));
    state.pending.upsert(CloudBatch {
        records: vec![local.clone()],
        ..Default::default()
    });
    let mut report = CloudSend::default();
    sending::failed(
        &mut state,
        &mut report,
        local.name.clone(),
        &engine::error(CKErrorCode::ServerRecordChanged),
    );
    assert_eq!(report.failure, Some(CloudSendFailure::Permanent));
    assert!(
        report.requeue.is_empty(),
        "an absent server copy cannot be immediately merged and requeued"
    );
    assert!(!state.records.contains_key(&local.name));
    assert!(state.pending.is_empty());
}

#[test]
fn deleting_an_already_missing_record_acknowledges_the_delete() {
    let local = record(1, "gone");
    let session = Session::new(3, |_, _| {});
    {
        let mut state = session.state.lock().unwrap();
        state
            .records
            .insert(local.name.clone(), records::build(&local, None));
        state.pending.upsert(CloudBatch {
            deleted: vec![local.name.clone()],
            ..Default::default()
        });
    }
    let report = sending::apply_results(
        &session,
        sending::SendResults {
            failed_deletes: vec![(
                records::record_id(&local.name),
                engine::error(CKErrorCode::UnknownItem),
            )],
            ..Default::default()
        },
    );
    assert_eq!(
        report,
        CloudSend {
            deleted: vec![local.name],
            ..Default::default()
        }
    );
    let state = session.state.lock().unwrap();
    assert!(state.pending.is_empty());
    assert!(state.records.is_empty());
}

#[test]
fn a_send_failure_drops_all_unacknowledged_attempts_but_keeps_newer_and_unsent_work() {
    for (code, expected) in [
        (CKErrorCode::QuotaExceeded, CloudSendFailure::QuotaExceeded),
        (CKErrorCode::InvalidArguments, CloudSendFailure::Permanent),
    ] {
        let a = record(1, "transient failure");
        let deleted = record(2, "delete failure");
        let saved = record(3, "accepted");
        let newer = record(3, "edited while sending");
        let unsent = record(4, "not in this attempt");
        let session = Session::new(4, |_, _| {});
        session.state.lock().unwrap().pending.upsert(CloudBatch {
            records: vec![a.clone(), newer.clone(), unsent.clone()],
            deleted: vec![deleted.name.clone()],
            full_fetch: false,
        });
        let report = sending::apply_results(
            &session,
            sending::SendResults {
                saved: vec![records::build(&saved, None)],
                failed_saves: vec![(
                    records::build(&a, None),
                    engine::error(CKErrorCode::NetworkFailure),
                )],
                failed_deletes: vec![(records::record_id(&deleted.name), engine::error(code))],
                ..Default::default()
            },
        );
        assert_eq!(report.failure, Some(expected));
        assert_eq!(report.saved, vec![saved]);
        let state = session.state.lock().unwrap();
        assert_eq!(
            state.pending.saves(),
            vec![newer, unsent],
            "the summary failure also covers the transient failure from this attempt"
        );
        assert!(state.pending.deletes().is_empty());
    }
}

#[test]
fn permanent_zone_failure_retains_records_and_requeues_the_zone_on_the_next_send() {
    let session = std::sync::Arc::new(Session::new(10, |_, _| {}));
    let pending = record(1, "not attempted yet");
    session.state.lock().unwrap().pending.upsert(CloudBatch {
        records: vec![pending.clone()],
        ..Default::default()
    });
    let report = sending::database_failures(&session, [CKErrorCode::QuotaExceeded]);
    assert_eq!(
        report,
        CloudSend {
            failure: Some(CloudSendFailure::QuotaExceeded),
            ..Default::default()
        }
    );
    assert_eq!(
        session.state.lock().unwrap().pending.saves(),
        vec![pending.clone()]
    );
    let engine = engine::Engine::default();
    super::requests::send(&engine, &session, CloudBatch::default());
    assert_eq!(
        engine.zones.get(),
        1,
        "the removed zone save must be requeued on the next send"
    );
    assert_eq!(session.state.lock().unwrap().pending.saves(), vec![pending]);
    super::requests::send(&engine, &session, CloudBatch::default());
    assert_eq!(engine.zones.get(), 1);
}

#[test]
fn acknowledgement_requeues_only_newer_work_and_never_a_losing_conflict_payload() {
    use crate::cloudkit::engine::SyncEngine;
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSDictionary, NSError, NSString};
    let losing = record(1, "local loser");
    let winning = record(1, "server winner");
    let saved = record(2, "older acknowledged edit");
    let newer = record(2, "newer pending edit");
    let session = Session::new(6, |_, _| {});
    session.state.lock().unwrap().pending.upsert(CloudBatch {
        records: vec![losing.clone(), newer.clone()],
        ..Default::default()
    });
    let server = records::build(&winning, None);
    let info = NSDictionary::<NSString, AnyObject>::from_slices(
        &[unsafe { objc2_cloud_kit::CKRecordChangedErrorServerRecordKey }],
        &[server.as_ref()],
    );
    let conflict = unsafe {
        NSError::errorWithDomain_code_userInfo(
            objc2_cloud_kit::CKErrorDomain,
            CKErrorCode::ServerRecordChanged.0,
            Some(&info),
        )
    };
    let report = sending::apply_results(
        &session,
        sending::SendResults {
            saved: vec![records::build(&saved, None)],
            failed_saves: vec![(records::build(&losing, None), conflict)],
            ..Default::default()
        },
    );
    assert_eq!(report.conflicts, vec![winning]);
    assert_eq!(
        session.state.lock().unwrap().pending.saves(),
        vec![newer.clone()]
    );
    let engine = engine::Engine::default();
    sending::queue_after_ack(&engine, &session, &report);
    let queued = engine
        .pending()
        .into_iter()
        .map(|change| unsafe { change.recordID().recordName() }.to_string())
        .collect::<Vec<_>>();
    assert_eq!(queued, vec![newer.name]);
}

#[test]
fn thousands_of_records_enter_the_engine_pending_set_and_native_batching_bounds_each_batch() {
    use crate::cloudkit::engine::SyncEngine;
    use objc2::{rc::autoreleasepool, AllocAnyThread};
    use objc2_cloud_kit::CKSyncEngineSendChangesScope;
    use objc2_foundation::NSSet;
    autoreleasepool(|_| {
        let session = std::sync::Arc::new(Session::new(7, |_, _| {}));
        let engine = engine::Engine::default();
        let records = (1..=2000)
            .map(|id| record(id, "payload"))
            .collect::<Vec<_>>();
        super::requests::send(
            &engine,
            &session,
            CloudBatch {
                records,
                ..Default::default()
            },
        );
        assert_eq!(engine.pending().len(), 2000);
        assert_eq!(engine.sends.get(), 1);
        let names = engine
            .pending()
            .into_iter()
            .filter_map(|change| records::name(&*unsafe { change.recordID() }))
            .collect();
        let scope = unsafe {
            CKSyncEngineSendChangesScope::initWithZoneIDs(
                CKSyncEngineSendChangesScope::alloc(),
                Some(&NSSet::from_retained_slice(&[records::zone_id()])),
            )
        };
        let batch = sending::build_batch(&session, &scope, &names).unwrap();
        let count = unsafe { batch.recordsToSave() }.count();
        assert!(
            count > 0 && count < 2000,
            "CloudKit must bound the request; got {count}"
        );
        assert_eq!(
            session.state.lock().unwrap().pending.saves().len(),
            2000,
            "building a batch is not an acknowledgment"
        );
    });
}

#[test]
fn transient_zone_save_failure_keeps_pending_for_the_engines_own_retry() {
    let session = std::sync::Arc::new(Session::new(11, |_, _| {}));
    let record = record(1, "retained");
    session.state.lock().unwrap().pending.upsert(CloudBatch {
        records: vec![record.clone()],
        ..Default::default()
    });
    assert_eq!(
        sending::database_failures(&session, [CKErrorCode::ZoneBusy]),
        CloudSend::default()
    );
    let engine = engine::Engine::default();
    super::requests::send(&engine, &session, CloudBatch::default());
    assert_eq!(engine.zones.get(), 0);
    assert_eq!(session.state.lock().unwrap().pending.saves(), vec![record]);
}
