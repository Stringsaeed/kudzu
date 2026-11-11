//! CKSyncEngine ownership. All server work is asynchronous; no adapter timer or worker.
use super::{delegate::Delegate, delivery::Delivery, pending::Pending, records, requests};
use block2::RcBlock;
use kudzu_app::{
    error::PortError,
    ports::{CloudBatch, CloudSend},
};
use kudzu_core::sync::CONTAINER_ID;
use objc2::{
    rc::{autoreleasepool, Retained},
    runtime::ProtocolObject,
    AllocAnyThread, ClassType,
};
use objc2_cloud_kit::{
    CKAccountStatus, CKContainer, CKRecord, CKRecordZone, CKSyncEngine,
    CKSyncEngineAccountChangeType, CKSyncEngineConfiguration, CKSyncEnginePendingZoneSave,
    CKSyncEngineStateSerialization,
};
use objc2_foundation::{
    NSArray, NSCopying, NSData, NSError, NSKeyedArchiver, NSKeyedUnarchiver, NSString,
};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver},
        Arc, Mutex, PoisonError,
    },
};

pub(super) enum Event {
    Fetched(CloudBatch),
    Sent(CloudSend),
    State(Vec<u8>),
    Account(CKSyncEngineAccountChangeType),
    Availability(CKAccountStatus),
    Failed(PortError),
    ZoneDeleted,
}

pub(super) struct Session {
    pub(super) delivery: Delivery<Event>,
    pub(super) state: Mutex<State>,
    pub(super) fetching: Mutex<super::fetching::Fetching>,
    status_request: AtomicU64,
}

#[derive(Default)]
pub(super) struct State {
    pub(super) pending: Pending,
    pub(super) records: BTreeMap<String, Retained<CKRecord>>,
    pub(super) zone_save_needed: bool,
}

// CKRecord does not promise general thread safety. The cache owns copies, never
// mutates them, and only reads/copies/releases them under Session::state's mutex.
// Outgoing batches receive separate copies, so CloudKit never saves a cached object.
unsafe impl Send for State {}

impl Session {
    pub(super) fn new(generation: u64, sink: impl Fn(u64, Event) + Send + Sync + 'static) -> Self {
        Self {
            delivery: Delivery::new(generation, sink),
            state: Mutex::new(State::default()),
            fetching: Mutex::new(super::fetching::Fetching::default()),
            status_request: AtomicU64::new(0),
        }
    }

    pub(super) fn reset(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.records.clear();
        state.pending.clear();
        state.zone_save_needed = false;
    }

    pub(super) fn account_changed(
        &self,
        change: CKSyncEngineAccountChangeType,
        refresh: impl FnOnce(),
    ) -> Event {
        if matches!(
            change,
            CKSyncEngineAccountChangeType::SignOut | CKSyncEngineAccountChangeType::SwitchAccounts
        ) {
            self.reset();
        }
        refresh();
        Event::Account(change)
    }

    pub(super) fn zone_deleted(&self, recreate: impl FnOnce()) -> Event {
        self.reset();
        recreate();
        Event::ZoneDeleted
    }

    pub(super) fn fetched(
        &self,
        records: impl IntoIterator<Item = Retained<CKRecord>>,
        deleted: impl IntoIterator<Item = (Retained<objc2_cloud_kit::CKRecordID>, String)>,
    ) -> CloudBatch {
        let mut batch = CloudBatch::default();
        let mut dropped = 0;
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        for record in records {
            if let Some(remote) = records::remote(&record) {
                state.records.insert(remote.name.clone(), record.copy());
                batch.records.push(remote);
            } else if records::our_zone(&*unsafe { record.recordID().zoneID() }) {
                dropped += 1;
            }
        }
        for (id, record_type) in deleted {
            if let Some(name) = records::deletion_name(&id, &record_type) {
                state.records.remove(&name);
                batch.deleted.push(name);
            } else if records::our_zone(&*unsafe { id.zoneID() }) {
                dropped += 1;
            }
        }
        drop(state);
        records::log_dropped(dropped);
        batch
    }
}

pub(super) struct Running {
    pub(super) engine: Retained<CKSyncEngine>,
    // The engine's delegate property is weak. Keep it through cancellation completion.
    _delegate: Retained<Delegate>,
    pub(super) session: Arc<Session>,
}

impl Running {
    /// Caller has checked the entitlement and retired the previous engine.
    pub(super) fn start(checkpoint: Option<Vec<u8>>, session: Arc<Session>) -> Self {
        autoreleasepool(|_| {
            // As in the reference, an unreadable archive starts a fresh fetch. It must not
            // strand sync in a retry loop on the same corrupt file. The service still owns
            // replacement of that file after the new fetch is durable.
            let saved = checkpoint
                .as_deref()
                .and_then(|bytes| decode_state(bytes).ok());
            let container =
                unsafe { CKContainer::containerWithIdentifier(&NSString::from_str(CONTAINER_ID)) };
            let database = unsafe { container.privateCloudDatabase() };
            let delegate = Delegate::new(session.clone(), container.clone());
            let engine = unsafe {
                let configuration =
                    CKSyncEngineConfiguration::initWithDatabase_stateSerialization_delegate(
                        CKSyncEngineConfiguration::alloc(),
                        &database,
                        saved.as_deref(),
                        ProtocolObject::from_ref(&*delegate),
                    );
                CKSyncEngine::initWithConfiguration(CKSyncEngine::alloc(), &configuration)
            };
            // Restored record intents are not authority to upload. The service resubmits
            // current payloads/deletions after its fetch/format gate; next_batch uses only those.
            requests::start(&*engine, saved.is_none(), || {
                availability(&container, &session)
            });
            Self {
                engine,
                _delegate: delegate,
                session,
            }
        })
    }

    pub(super) fn fetch(&self) {
        autoreleasepool(|_| {
            requests::fetch(&*self.engine, &self.session);
        })
    }

    pub(super) fn send(&self, batch: CloudBatch) {
        autoreleasepool(|_| {
            requests::send(&*self.engine, &self.session, batch);
        })
    }

    pub(super) fn cancel(self: Arc<Self>) -> Receiver<()> {
        autoreleasepool(|_| {
            self.session.delivery.stop();
            let (sender, receiver) = mpsc::channel();
            let keep_alive = self.clone();
            let completion = RcBlock::new(move || {
                autoreleasepool(|_| {
                    // Do not release the weak delegate while CloudKit is cancelling.
                    let _ = &keep_alive;
                    let _ = sender.send(());
                })
            });
            unsafe {
                self.engine
                    .cancelOperationsWithCompletionHandler(Some(&completion));
            }
            receiver
        })
    }
}

pub(super) fn queue_zone(engine: &CKSyncEngine) {
    autoreleasepool(|_| unsafe {
        let zone = CKRecordZone::initWithZoneID(CKRecordZone::alloc(), &records::zone_id());
        let save =
            CKSyncEnginePendingZoneSave::initWithZone(CKSyncEnginePendingZoneSave::alloc(), &zone);
        engine
            .state()
            .addPendingDatabaseChanges(&NSArray::from_retained_slice(&[Retained::into_super(
                save,
            )]));
    })
}

pub(super) fn availability(container: &CKContainer, session: &Arc<Session>) {
    autoreleasepool(|_| {
        let request = session.status_request.fetch_add(1, Ordering::SeqCst) + 1;
        let session = session.clone();
        let completion = RcBlock::new(move |status: CKAccountStatus, _error: *mut NSError| {
            autoreleasepool(|_| {
                let Some(lease) = session.delivery.enter() else {
                    return;
                };
                if session.status_request.load(Ordering::SeqCst) == request {
                    lease.emit(Event::Availability(status));
                }
            })
        });
        unsafe {
            container.accountStatusWithCompletionHandler(&completion);
        }
    })
}

pub(super) fn decode_state(
    bytes: &[u8],
) -> Result<Retained<CKSyncEngineStateSerialization>, PortError> {
    autoreleasepool(|_| {
        if bytes.is_empty() {
            return Err(PortError::InvalidData);
        }
        let object = unsafe {
            NSKeyedUnarchiver::unarchivedObjectOfClass_fromData_error(
                CKSyncEngineStateSerialization::class(),
                &NSData::with_bytes(bytes),
            )
        }
        .map_err(|_| PortError::InvalidData)?;
        object
            .downcast::<CKSyncEngineStateSerialization>()
            .map_err(|_| PortError::InvalidData)
    })
}

pub(super) fn encode_state(state: &CKSyncEngineStateSerialization) -> Result<Vec<u8>, PortError> {
    autoreleasepool(|_| {
        let data = unsafe {
            NSKeyedArchiver::archivedDataWithRootObject_requiringSecureCoding_error(state, true)
        }
        .map_err(|_| PortError::InvalidData)?;
        Ok(data.to_vec())
    })
}
