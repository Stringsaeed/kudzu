//! Batch construction and server acknowledgments; merges belong to the sync service.
use super::{engine::SyncEngine, errors, native::Session, records};
use block2::StackBlock;
use kudzu_app::ports::CloudSend;
use objc2::{rc::Retained, AllocAnyThread};
use objc2_cloud_kit::{
    CKErrorCode, CKRecord, CKRecordChangedErrorServerRecordKey, CKRecordID,
    CKSyncEnginePendingRecordZoneChange, CKSyncEnginePendingRecordZoneChangeType,
    CKSyncEngineRecordZoneChangeBatch, CKSyncEngineSendChangesContext,
    CKSyncEngineSentRecordZoneChangesEvent,
};
use objc2_foundation::{NSArray, NSCopying, NSError};
use std::{collections::BTreeMap, ptr::NonNull, sync::PoisonError};

fn pending_change(
    name: &str,
    kind: CKSyncEnginePendingRecordZoneChangeType,
) -> Retained<CKSyncEnginePendingRecordZoneChange> {
    unsafe {
        CKSyncEnginePendingRecordZoneChange::initWithRecordID_type(
            CKSyncEnginePendingRecordZoneChange::alloc(),
            &records::record_id(name),
            kind,
        )
    }
}

pub(super) fn queue_pending(engine: &impl SyncEngine, session: &Session) {
    queue_selected(engine, session, |_| true);
}

pub(super) fn queue_after_ack(engine: &impl SyncEngine, session: &Session, report: &CloudSend) {
    queue_selected(engine, session, |name| {
        report.saved.iter().any(|r| r.name == name) || report.deleted.iter().any(|r| r == name)
    });
}

fn queue_selected(engine: &impl SyncEngine, session: &Session, include: impl Fn(&str) -> bool) {
    let changes = {
        let state = session.state.lock().unwrap_or_else(PoisonError::into_inner);
        state
            .pending
            .saves()
            .iter()
            .filter(|r| include(&r.name))
            .map(|r| pending_change(&r.name, CKSyncEnginePendingRecordZoneChangeType::SaveRecord))
            .chain(
                state
                    .pending
                    .deletes()
                    .iter()
                    .filter(|name| include(name))
                    .map(|name| {
                        pending_change(name, CKSyncEnginePendingRecordZoneChangeType::DeleteRecord)
                    }),
            )
            .collect::<Vec<_>>()
    };
    if !changes.is_empty() {
        engine.add(&changes);
    }
}

pub(super) fn next_batch(
    session: &Session,
    engine: &impl SyncEngine,
    context: &CKSyncEngineSendChangesContext,
) -> Option<Retained<CKSyncEngineRecordZoneChangeBatch>> {
    let scope = unsafe { context.options().scope() };
    let scheduled = engine.pending();
    let scheduled_names = scheduled
        .iter()
        .filter_map(|change| records::name(&*unsafe { change.recordID() }))
        .collect::<std::collections::BTreeSet<_>>();
    build_batch(session, &scope, &scheduled_names)
}

pub(super) fn build_batch(
    session: &Session,
    scope: &objc2_cloud_kit::CKSyncEngineSendChangesScope,
    scheduled_names: &std::collections::BTreeSet<String>,
) -> Option<Retained<CKSyncEngineRecordZoneChangeBatch>> {
    let (built, pending) = {
        let state = session.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut built = BTreeMap::new();
        let mut pending = Vec::new();
        for remote in state.pending.saves() {
            let change = pending_change(
                &remote.name,
                CKSyncEnginePendingRecordZoneChangeType::SaveRecord,
            );
            if scheduled_names.contains(&remote.name)
                && unsafe { scope.containsPendingRecordZoneChange(&change) }
            {
                built.insert(
                    remote.name.clone(),
                    records::build(&remote, state.records.get(&remote.name).map(|r| &**r)),
                );
                pending.push(change);
            }
        }
        for name in state.pending.deletes() {
            let change =
                pending_change(&name, CKSyncEnginePendingRecordZoneChangeType::DeleteRecord);
            if scheduled_names.contains(&name)
                && unsafe { scope.containsPendingRecordZoneChange(&change) }
            {
                pending.push(change);
            }
        }
        (built, pending)
    };
    if pending.is_empty() {
        return None;
    }
    // The initializer invokes this immutable provider during construction and retains
    // its records. It handles CloudKit's batch count/size limits.
    let provider = StackBlock::new(|id: NonNull<CKRecordID>| -> *mut CKRecord {
        records::name(unsafe { id.as_ref() })
            .and_then(|name| built.get(&name))
            .map_or(std::ptr::null_mut(), |record| {
                Retained::as_ptr(record).cast_mut()
            })
    });
    unsafe {
        CKSyncEngineRecordZoneChangeBatch::initWithPendingChanges_recordProvider(
            CKSyncEngineRecordZoneChangeBatch::alloc(),
            &NSArray::from_retained_slice(&pending),
            &provider,
        )
    }
}

#[derive(Default)]
pub(super) struct SendResults {
    pub(super) saved: Vec<Retained<CKRecord>>,
    pub(super) deleted: Vec<Retained<CKRecordID>>,
    pub(super) failed_saves: Vec<(Retained<CKRecord>, Retained<NSError>)>,
    pub(super) failed_deletes: Vec<(Retained<CKRecordID>, Retained<NSError>)>,
}

pub(super) fn sent(session: &Session, event: &CKSyncEngineSentRecordZoneChangesEvent) -> CloudSend {
    let results = unsafe {
        let deletes = event.failedRecordDeletes();
        SendResults {
            saved: event.savedRecords().iter().collect(),
            deleted: event.deletedRecordIDs().iter().collect(),
            failed_saves: event
                .failedRecordSaves()
                .iter()
                .map(|failure| (failure.record(), failure.error()))
                .collect(),
            failed_deletes: deletes
                .keys()
                .filter_map(|id| deletes.objectForKey(&id).map(|error| (id, error)))
                .collect(),
        }
    };
    apply_results(session, results)
}

pub(super) fn apply_results(session: &Session, results: SendResults) -> CloudSend {
    let mut report = CloudSend::default();
    let mut failed_names = Vec::new();
    let mut dropped = 0;
    let mut state = session.state.lock().unwrap_or_else(PoisonError::into_inner);
    for record in results.saved {
        if let Some(remote) = records::remote(&record) {
            state.pending.saved(&remote);
            state.records.insert(remote.name.clone(), record.copy());
            report.saved.push(remote);
        } else if records::our_zone(&*unsafe { record.recordID().zoneID() }) {
            dropped += 1;
        }
    }
    for id in results.deleted {
        if let Some(name) = records::name(&id) {
            state.pending.deleted(&name);
            state.records.remove(&name);
            report.deleted.push(name);
        } else if records::our_zone(&*unsafe { id.zoneID() }) {
            dropped += 1;
        }
    }
    for (record, error) in results.failed_saves {
        if let Some((_, name)) = records::identity(&record) {
            failed_names.push(name.clone());
            failed(&mut state, &mut report, name, &error);
        } else if records::our_zone(&*unsafe { record.recordID().zoneID() }) {
            dropped += 1;
        }
    }
    for (id, error) in results.failed_deletes {
        if let Some(name) = records::name(&id) {
            failed_names.push(name.clone());
            if errors::code(&error) == CKErrorCode::UnknownItem {
                state.records.remove(&name);
                state.pending.deleted(&name);
                report.deleted.push(name);
            } else {
                failed(&mut state, &mut report, name, &error);
            }
        } else if records::our_zone(&*unsafe { id.zoneID() }) {
            dropped += 1;
        }
    }
    if report.failure.is_some() {
        // A summary failure covers every unacknowledged record in this attempt,
        // including any retryable per-record error beside the permanent one.
        for name in failed_names {
            if !report.saved.iter().any(|r| r.name == name) && !report.deleted.contains(&name) {
                state.pending.remove(&name);
            }
        }
    }
    drop(state);
    records::log_dropped(dropped);
    report
}

pub(super) fn failed(
    state: &mut super::native::State,
    report: &mut CloudSend,
    name: String,
    error: &NSError,
) {
    match errors::code(error) {
        CKErrorCode::ServerRecordChanged => {
            state.pending.remove(&name);
            let server = server_record(error);
            if server
                .as_ref()
                .is_some_and(|record| records::identity(record).is_none())
            {
                records::log_dropped(1);
            }
            let server = server.and_then(|server| {
                records::remote(&server)
                    .filter(|remote| remote.name == name)
                    .map(|remote| (server, remote))
            });
            if let Some((server, remote)) = server {
                state.records.insert(name.clone(), server.copy());
                report.conflicts.push(remote);
                report.requeue.push(name);
            } else {
                // No server copy means there is nothing safe to merge. Report a
                // permanent failure and discard the stale tag; a service retry starts fresh.
                state.records.remove(&name);
                errors::send_failure(report, CKErrorCode::ServerRecordChanged);
            }
        }
        CKErrorCode::ZoneNotFound | CKErrorCode::UnknownItem => {
            report.zone_missing |= errors::code(error) == CKErrorCode::ZoneNotFound;
            state.records.remove(&name);
            state.pending.remove(&name);
            report.requeue.push(name);
        }
        code => {
            if errors::send_failure(report, code) {
                state.pending.remove(&name);
            }
        }
    }
}

pub(super) fn database_failures(
    session: &Session,
    codes: impl IntoIterator<Item = CKErrorCode>,
) -> CloudSend {
    let mut report = CloudSend::default();
    for code in codes {
        errors::send_failure(&mut report, code);
    }
    if report.failure.is_some() {
        // No record was attempted by a failed database operation. Retain payloads;
        // the service's next (possibly empty) send requeues the zone before flushing.
        session
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .zone_save_needed = true;
    }
    report
}

fn server_record(error: &NSError) -> Option<Retained<CKRecord>> {
    error
        .userInfo()
        .objectForKey(unsafe { CKRecordChangedErrorServerRecordKey })?
        .downcast::<CKRecord>()
        .ok()
}
