//! Serial CloudKit delegate events, translated before the service is notified.
use super::{
    errors,
    native::{self, Event, Session},
    records, sending,
};
use kudzu_app::ports::CloudBatch;
use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{NSObject, NSObjectProtocol},
    AllocAnyThread, DefinedClass,
};
use objc2_cloud_kit::{
    CKContainer, CKSyncEngine, CKSyncEngineDelegate, CKSyncEngineEvent, CKSyncEngineEventType,
    CKSyncEngineRecordZoneChangeBatch, CKSyncEngineSendChangesContext,
};
use std::sync::Arc;

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
            let session = &self.ivars().session;
            let Some(lease) = session.delivery.enter() else {
                return;
            };
            if let Some(output) = translate(session, &self.ivars().container, engine, event) {
                lease.emit(output);
            }
        }

        #[unsafe(method_id(syncEngine:nextRecordZoneChangeBatchForContext:))]
        fn next_batch(
            &self,
            engine: &CKSyncEngine,
            context: &CKSyncEngineSendChangesContext,
        ) -> Option<Retained<CKSyncEngineRecordZoneChangeBatch>> {
            let session = &self.ivars().session;
            session
                .delivery
                .enter()
                .and_then(|_lease| sending::next_batch(session, engine, context))
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
            CKSyncEngineEventType::StateUpdate => Some(
                match native::encode_state(&event.stateUpdateEvent().stateSerialization()) {
                    Ok(bytes) => Event::State(bytes),
                    Err(error) => Event::Failed(error),
                },
            ),
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
                    fetched.deletions().iter().map(|d| d.recordID()),
                );
                (!batch.records.is_empty() || !batch.deleted.is_empty())
                    .then_some(Event::Fetched(batch))
            }
            CKSyncEngineEventType::DidFetchChanges => Some(Event::Fetched(CloudBatch {
                full_fetch: true,
                ..Default::default()
            })),
            CKSyncEngineEventType::DidFetchRecordZoneChanges => {
                let fetched = event.didFetchRecordZoneChangesEvent();
                if !records::our_zone(&fetched.zoneID()) {
                    return None;
                }
                let error = fetched.error()?;
                let code = errors::code(&error);
                (!errors::transient(code)).then(|| Event::Failed(errors::port(code)))
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
                let mut report = Default::default();
                for failure in event.sentDatabaseChangesEvent().failedZoneSaves() {
                    if records::our_zone(&failure.recordZone().zoneID()) {
                        errors::send_failure(&mut report, errors::code(&failure.error()));
                    }
                }
                (report != Default::default()).then_some(Event::Sent(report))
            }
            _ => None,
        }
    }
}
