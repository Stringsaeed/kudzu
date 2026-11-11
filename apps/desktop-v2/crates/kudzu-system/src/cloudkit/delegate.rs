//! Serial CloudKit delegate events, translated before the service is notified.
use super::{
    errors,
    native::{self, Event, Session},
    records, sending,
};
use objc2::{
    define_class, msg_send,
    rc::{autoreleasepool, Retained},
    runtime::{NSObject, NSObjectProtocol},
    AllocAnyThread, DefinedClass,
};
use objc2_cloud_kit::{
    CKContainer, CKSyncEngine, CKSyncEngineDelegate, CKSyncEngineEvent, CKSyncEngineEventType,
    CKSyncEngineRecordZoneChangeBatch, CKSyncEngineSendChangesContext,
};
use std::sync::{Arc, PoisonError};

pub(super) struct Ivars {
    session: Arc<Session>,
    container: Retained<CKContainer>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "KudzuRuneCloudDelegate"]
    #[ivars = Ivars]
    pub(super) struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}
    unsafe impl CKSyncEngineDelegate for Delegate {
        #[unsafe(method(syncEngine:handleEvent:))]
        fn handle_event(&self, engine: &CKSyncEngine, event: &CKSyncEngineEvent) {
            autoreleasepool(|_| {
                let session = &self.ivars().session;
                let Some(lease) = session.delivery.enter() else {
                    return;
                };
                if let Some(output) = translate(session, &self.ivars().container, engine, event) {
                    lease.emit(output);
                }
            })
        }

        #[unsafe(method_id(syncEngine:nextRecordZoneChangeBatchForContext:))]
        fn next_batch(
            &self,
            engine: &CKSyncEngine,
            context: &CKSyncEngineSendChangesContext,
        ) -> Option<Retained<CKSyncEngineRecordZoneChangeBatch>> {
            autoreleasepool(|_| {
                let session = &self.ivars().session;
                session
                    .delivery
                    .enter()
                    .and_then(|_lease| sending::next_batch(session, engine, context))
            })
        }
    }
);

// The ivars hold synchronized Rust state and a Send + Sync CKContainer. NSObject is
// used only for ownership and dispatch, without mutable cross-thread properties.
unsafe impl Send for Delegate {}
unsafe impl Sync for Delegate {}

impl Delegate {
    pub(super) fn new(session: Arc<Session>, container: Retained<CKContainer>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(Ivars { session, container });
        unsafe { msg_send![super(this), init] }
    }
}

fn translate(
    session: &Arc<Session>,
    container: &CKContainer,
    engine: &CKSyncEngine,
    event: &CKSyncEngineEvent,
) -> Option<Event> {
    // Each typed event accessor below is used only after checking its discriminator.
    unsafe {
        match event.r#type() {
            CKSyncEngineEventType::StateUpdate => state_event(native::encode_state(
                &event.stateUpdateEvent().stateSerialization(),
            )),
            CKSyncEngineEventType::AccountChange => {
                let change = event.accountChangeEvent().changeType();
                Some(session.account_changed(change, || native::availability(container, session)))
            }
            CKSyncEngineEventType::FetchedDatabaseChanges => {
                let ours = event
                    .fetchedDatabaseChangesEvent()
                    .deletions()
                    .iter()
                    .any(|deletion| records::our_zone(&deletion.zoneID()));
                if !ours {
                    return None;
                }
                Some(session.zone_deleted(|| native::queue_zone(engine)))
            }
            CKSyncEngineEventType::FetchedRecordZoneChanges => {
                let fetched = event.fetchedRecordZoneChangesEvent();
                let batch = session.fetched(
                    fetched.modifications().iter(),
                    fetched
                        .deletions()
                        .iter()
                        .map(|d| (d.recordID(), d.recordType().to_string())),
                );
                (!batch.records.is_empty() || !batch.deleted.is_empty())
                    .then_some(Event::Fetched(batch))
            }
            CKSyncEngineEventType::WillFetchChanges => {
                session
                    .fetching
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .began();
                None
            }
            CKSyncEngineEventType::DidFetchChanges => did_fetch(session),
            CKSyncEngineEventType::DidFetchRecordZoneChanges => {
                let fetched = event.didFetchRecordZoneChangesEvent();
                if !records::our_zone(&fetched.zoneID()) {
                    return None;
                }
                zone_fetch_result(
                    session,
                    fetched.error().as_deref().map(errors::code),
                    || native::queue_zone(engine),
                )
            }
            CKSyncEngineEventType::SentRecordZoneChanges => {
                let report = sending::sent(session, &event.sentRecordZoneChangesEvent());
                if report.zone_missing {
                    native::queue_zone(engine);
                }
                // The engine may have removed the pending ID for an older save while
                // the service was submitting a newer payload with that same name.
                sending::queue_after_ack(engine, session, &report);
                (report != Default::default()).then_some(Event::Sent(report))
            }
            CKSyncEngineEventType::SentDatabaseChanges => {
                let report = sending::database_failures(
                    session,
                    event
                        .sentDatabaseChangesEvent()
                        .failedZoneSaves()
                        .iter()
                        .filter(|failure| records::our_zone(&failure.recordZone().zoneID()))
                        .map(|failure| errors::code(&failure.error())),
                );
                (report != Default::default()).then_some(Event::Sent(report))
            }
            _ => None,
        }
    }
}

pub(super) fn zone_fetch_result(
    session: &Session,
    error: Option<objc2_cloud_kit::CKErrorCode>,
    recreate: impl FnOnce(),
) -> Option<Event> {
    let code = error?;
    session
        .fetching
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .zone_failed();
    if code == objc2_cloud_kit::CKErrorCode::ZoneNotFound {
        recreate();
        return None;
    }
    (!errors::transient(code)).then(|| Event::Failed(errors::port(code)))
}

pub(super) fn did_fetch(session: &Session) -> Option<Event> {
    session
        .fetching
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .delegate_finished()
}

pub(super) fn state_event(result: Result<Vec<u8>, kudzu_app::error::PortError>) -> Option<Event> {
    Some(match result {
        Ok(bytes) if bytes.is_empty() => return None,
        Ok(bytes) => Event::State(bytes),
        Err(error) => Event::Failed(error),
    })
}
