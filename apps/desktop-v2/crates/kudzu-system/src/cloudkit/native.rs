//! CKSyncEngine ownership. All server work is asynchronous; no adapter timer or worker.
use super::{delegate::Delegate, delivery::Delivery, errors, pending::Pending, records, sending};
use block2::RcBlock;
use kudzu_app::{
    error::PortError,
    ports::{CloudBatch, CloudSend},
};
use kudzu_core::sync::CONTAINER_ID;
use objc2::{rc::Retained, runtime::ProtocolObject, AllocAnyThread, ClassType};
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
    status_request: AtomicU64,
}

#[derive(Default)]
pub(super) struct State {
    pub(super) pending: Pending,
    pub(super) records: BTreeMap<String, Retained<CKRecord>>,
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
            status_request: AtomicU64::new(0),
        }
    }

    pub(super) fn reset(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.records.clear();
        state.pending.clear();
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
        deleted: impl IntoIterator<Item = Retained<objc2_cloud_kit::CKRecordID>>,
    ) -> CloudBatch {
        let mut batch = CloudBatch::default();
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        for record in records {
            if let Some(remote) = records::remote(&record) {
                state.records.insert(remote.name.clone(), record.copy());
                batch.records.push(remote);
            }
        }
        for id in deleted {
            if let Some(name) = records::name(&id) {
                state.records.remove(&name);
                batch.deleted.push(name);
            }
        }
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
        if saved.is_none() {
            queue_zone(&engine);
        }
        availability(&container, &session);
        Self {
            engine,
            _delegate: delegate,
            session,
        }
    }

    pub(super) fn fetch(&self) {
        let session = self.session.clone();
        let completion = RcBlock::new(move |error: *mut NSError| {
            let Some(lease) = session.delivery.enter() else {
                return;
            };
            if let Some(error) = unsafe { error.as_ref() } {
                let code = errors::code(error);
                if !errors::transient(code) {
                    lease.emit(Event::Failed(errors::port(code)));
                }
            }
        });
        unsafe {
            self.engine
                .fetchChangesWithCompletionHandler(Some(&completion));
        }
    }

    pub(super) fn send(&self, batch: CloudBatch) {
        self.session
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pending
            .upsert(batch);
        sending::queue_pending(&self.engine, &self.session);
        let session = self.session.clone();
        let completion = RcBlock::new(move |error: *mut NSError| {
            let Some(lease) = session.delivery.enter() else {
                return;
            };
            if let Some(error) = unsafe { error.as_ref() } {
                let mut report = CloudSend::default();
                if errors::send_failure(&mut report, errors::code(error)) {
                    lease.emit(Event::Sent(report));
                }
            }
        });
        unsafe {
            self.engine
                .sendChangesWithCompletionHandler(Some(&completion));
        }
    }

    pub(super) fn cancel(self: Arc<Self>) -> Receiver<()> {
        self.session.delivery.stop();
        let (sender, receiver) = mpsc::channel();
        let keep_alive = self.clone();
        let completion = RcBlock::new(move || {
            // Do not release the weak delegate while CloudKit is cancelling.
            let _ = &keep_alive;
            let _ = sender.send(());
        });
        unsafe {
            self.engine
                .cancelOperationsWithCompletionHandler(Some(&completion));
        }
        receiver
    }
}

pub(super) fn queue_zone(engine: &CKSyncEngine) {
    unsafe {
        let zone = CKRecordZone::initWithZoneID(CKRecordZone::alloc(), &records::zone_id());
        let save =
            CKSyncEnginePendingZoneSave::initWithZone(CKSyncEnginePendingZoneSave::alloc(), &zone);
        engine
            .state()
            .addPendingDatabaseChanges(&NSArray::from_retained_slice(&[Retained::into_super(
                save,
            )]));
    }
}

pub(super) fn availability(container: &CKContainer, session: &Arc<Session>) {
    let request = session.status_request.fetch_add(1, Ordering::SeqCst) + 1;
    let session = session.clone();
    let completion = RcBlock::new(move |status: CKAccountStatus, _error: *mut NSError| {
        let Some(lease) = session.delivery.enter() else {
            return;
        };
        if session.status_request.load(Ordering::SeqCst) == request {
            lease.emit(Event::Availability(status));
        }
    });
    unsafe {
        container.accountStatusWithCompletionHandler(&completion);
    }
}

pub(super) fn decode_state(
    bytes: &[u8],
) -> Result<Retained<CKSyncEngineStateSerialization>, PortError> {
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
}

pub(super) fn encode_state(state: &CKSyncEngineStateSerialization) -> Result<Vec<u8>, PortError> {
    let data = unsafe {
        NSKeyedArchiver::archivedDataWithRootObject_requiringSecureCoding_error(state, true)
    }
    .map_err(|_| PortError::InvalidData)?;
    let bytes = data.to_vec();
    if bytes.is_empty() {
        Err(PortError::InvalidData)
    } else {
        Ok(bytes)
    }
}
