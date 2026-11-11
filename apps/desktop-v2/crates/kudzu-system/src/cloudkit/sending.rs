//! Batch construction and server acknowledgments; merges belong to the sync service.
use super::{errors, native::Session, records};
use block2::StackBlock;
use kudzu_app::ports::CloudSend;
use objc2::{rc::Retained, AllocAnyThread};
use objc2_cloud_kit::{
    CKErrorCode, CKRecord, CKRecordChangedErrorServerRecordKey, CKRecordID, CKSyncEngine,
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

pub(super) fn queue_pending(engine: &CKSyncEngine, session: &Session) {
    queue_selected(engine, session, |_| true);
}

pub(super) fn queue_after_ack(engine: &CKSyncEngine, session: &Session, report: &CloudSend) {
    queue_selected(engine, session, |name| {
        report.saved.iter().any(|r| r.name == name) || report.deleted.iter().any(|r| r == name)
    });
}

fn queue_selected(engine: &CKSyncEngine, session: &Session, include: impl Fn(&str) -> bool) {
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
        unsafe {
            engine
                .state()
                .addPendingRecordZoneChanges(&NSArray::from_retained_slice(&changes));
        }
    }
}

pub(super) fn next_batch(
    session: &Session,
    engine: &CKSyncEngine,
    context: &CKSyncEngineSendChangesContext,
) -> Option<Retained<CKSyncEngineRecordZoneChangeBatch>> {
    let scope = unsafe { context.options().scope() };
    let scheduled = unsafe { engine.state().pendingRecordZoneChanges() };
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

pub(super) fn sent(session: &Session, event: &CKSyncEngineSentRecordZoneChangesEvent) -> CloudSend {
    let mut report = CloudSend::default();
    let mut state = session.state.lock().unwrap_or_else(PoisonError::into_inner);
    unsafe {
        for record in event.savedRecords() {
            if let Some(remote) = records::remote(&record) {
                state.pending.saved(&remote);
                state.records.insert(remote.name.clone(), record.copy());
                report.saved.push(remote);
            }
        }
        for id in event.deletedRecordIDs() {
            if let Some(name) = records::name(&id) {
                state.pending.deleted(&name);
                state.records.remove(&name);
                report.deleted.push(name);
            }
        }
        for failure in event.failedRecordSaves() {
            if let Some(name) = records::name(&failure.record().recordID()) {
                failed(&mut state, &mut report, name, &failure.error());
            }
        }
        let deletes = event.failedRecordDeletes();
        for id in deletes.keys() {
            if let (Some(name), Some(error)) = (records::name(&id), deletes.objectForKey(&id)) {
                failed(&mut state, &mut report, name, &error);
            }
        }
    }
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
            if let Some(server) = server_record(error).filter(|server| {
                records::name(&*unsafe { server.recordID() }).as_ref() == Some(&name)
            }) {
                if let Some(remote) = records::remote(&server) {
                    report.conflicts.push(remote);
                }
                state.records.insert(name.clone(), server.copy());
            }
            state.pending.remove(&name);
            report.requeue.push(name);
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

fn server_record(error: &NSError) -> Option<Retained<CKRecord>> {
    error
        .userInfo()
        .objectForKey(unsafe { CKRecordChangedErrorServerRecordKey })?
        .downcast::<CKRecord>()
        .ok()
}
