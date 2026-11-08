//! CloudKit layer: a `CKSyncEngine` on the private database of `iCloud.sh.saeed.kudzu`.
//!
//! The engine owns change tokens, retries, batching and scheduling. This file translates its
//! events into `kudzu_core::sync` calls and back.
//!
//! Locking: the library (`core.store`) is always locked before `STATE`, and no lock is held
//! while calling into CloudKit (it can call the delegate back synchronously).

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use block2::{RcBlock, StackBlock};
use kudzu_core::sync::{self, RecordKind, RemoteRecord, SyncIndex};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AllocAnyThread, ClassType};
use objc2_cloud_kit::{
    CKContainer, CKCurrentUserDefaultName, CKErrorCode, CKRecord, CKRecordChangedErrorServerRecordKey, CKRecordID,
    CKRecordZone, CKRecordZoneID, CKSyncEngine, CKSyncEngineAccountChangeType, CKSyncEngineConfiguration,
    CKSyncEngineDelegate, CKSyncEngineEvent, CKSyncEngineEventType, CKSyncEnginePendingDatabaseChange,
    CKSyncEnginePendingRecordZoneChange, CKSyncEnginePendingRecordZoneChangeType, CKSyncEnginePendingZoneSave,
    CKSyncEngineRecordZoneChangeBatch, CKSyncEngineSendChangesContext, CKSyncEngineStateSerialization,
};
use objc2_foundation::{NSArray, NSData, NSError, NSKeyedArchiver, NSKeyedUnarchiver, NSString};
use parking_lot::Mutex;

use super::{publish, SyncStatus};
use crate::backend::core::core;

/// The iCloud container the Swift app already uses; same team, same bundle id.
pub const CONTAINER_ID: &str = "iCloud.sh.saeed.kudzu";
const FETCH_INTERVAL: Duration = Duration::from_secs(3 * 60);
const USAGE_INTERVAL: Duration = Duration::from_secs(10 * 60);

// ── State ────────────────────────────────────────────────────────────────────

struct Running {
    engine: Retained<CKSyncEngine>,
    _delegate: Retained<SyncDelegate>,
    /// Last server copy of each record; reusing it keeps CloudKit's change tag so saves of
    /// records we've seen don't conflict with ourselves.
    records: HashMap<String, Retained<CKRecord>>,
    index: SyncIndex,
    dir: PathBuf,
}

// CloudKit objects are used from the engine's queue and our threads, always behind STATE.
unsafe impl Send for Running {}

static STATE: Mutex<Option<Running>> = Mutex::new(None);
static STATUS: Mutex<SyncStatus> = Mutex::new(SyncStatus::Off);
/// Whether the most recent send hit a non-transient failure (keeps the error visible).
static LAST_SEND_FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Bumped on every start/stop so stale timer threads exit.
static GENERATION: AtomicU64 = AtomicU64::new(0);

pub(super) fn set_status(status: SyncStatus) {
    *STATUS.lock() = status;
}

pub(super) fn status() -> SyncStatus {
    STATUS.lock().clone()
}

fn engine() -> Option<Retained<CKSyncEngine>> {
    STATE.lock().as_ref().map(|r| r.engine.clone())
}

fn zone_id() -> Retained<CKRecordZoneID> {
    unsafe {
        CKRecordZoneID::initWithZoneName_ownerName(
            CKRecordZoneID::alloc(),
            &NSString::from_str(sync::ZONE_NAME),
            CKCurrentUserDefaultName,
        )
    }
}

fn record_id(name: &str) -> Retained<CKRecordID> {
    unsafe { CKRecordID::initWithRecordName_zoneID(CKRecordID::alloc(), &NSString::from_str(name), &zone_id()) }
}

fn in_our_zone(id: &CKRecordID) -> bool {
    unsafe { id.zoneID().zoneName() }.to_string() == sync::ZONE_NAME
}

// ── Entitlement check ────────────────────────────────────────────────────────

#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn SecTaskCreateFromSelf(allocator: *const c_void) -> *mut c_void;
    fn SecTaskCopyValueForEntitlement(task: *mut c_void, entitlement: *const c_void, error: *mut *mut c_void) -> *mut AnyObject;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
}

/// True when this process is signed with the iCloud container entitlement. Touching
/// CloudKit without it raises an Objective-C exception, so this gates everything.
fn entitled() -> bool {
    unsafe {
        let task = SecTaskCreateFromSelf(std::ptr::null());
        if task.is_null() {
            return false;
        }
        let key = NSString::from_str("com.apple.developer.icloud-container-identifiers");
        let value = SecTaskCopyValueForEntitlement(task, Retained::as_ptr(&key).cast(), std::ptr::null_mut());
        CFRelease(task);
        let Some(value) = Retained::from_raw(value) else { return false };
        let Ok(list) = value.downcast::<NSArray>() else { return false };
        list.iter().any(|v| v.downcast_ref::<NSString>().is_some_and(|s| s.to_string() == CONTAINER_ID))
    }
}

// ── Persistence ──────────────────────────────────────────────────────────────

fn sync_dir() -> PathBuf {
    let dir = crate::backend::identity::data_dir().join("sync");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn load_index(dir: &Path) -> SyncIndex {
    std::fs::read_to_string(dir.join("index.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_index(dir: &Path, index: &SyncIndex) {
    let tmp = dir.join("index.json.tmp");
    if std::fs::write(&tmp, serde_json::to_vec(index).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(tmp, dir.join("index.json"));
    }
}

fn load_engine_state(dir: &Path) -> Option<Retained<CKSyncEngineStateSerialization>> {
    let bytes = std::fs::read(dir.join("engine-state.data")).ok()?;
    let data = NSData::with_bytes(&bytes);
    let obj = unsafe { NSKeyedUnarchiver::unarchivedObjectOfClass_fromData_error(CKSyncEngineStateSerialization::class(), &data) }.ok()?;
    obj.downcast::<CKSyncEngineStateSerialization>().ok()
}

fn save_engine_state(dir: &Path, state: &CKSyncEngineStateSerialization) {
    match unsafe { NSKeyedArchiver::archivedDataWithRootObject_requiringSecureCoding_error(state, true) } {
        Ok(data) => {
            let tmp = dir.join("engine-state.data.tmp");
            if std::fs::write(&tmp, data.to_vec()).is_ok() {
                let _ = std::fs::rename(tmp, dir.join("engine-state.data"));
            }
        }
        Err(e) => log::error!("couldn't archive sync state: {}", e.localizedDescription()),
    }
}

/// Forget everything about the server (account switched or zone deleted). Local data stays.
fn reset_server_knowledge(dir: &Path, running: &mut Running) {
    running.index = SyncIndex::default();
    running.records.clear();
    save_index(dir, &running.index);
}

// ── Record conversion ────────────────────────────────────────────────────────

fn payload_of(record: &CKRecord) -> Option<String> {
    let key = NSString::from_str(sync::PAYLOAD_FIELD);
    let value = unsafe { record.objectForKey(&key) }?;
    let any: Retained<AnyObject> = unsafe { Retained::cast_unchecked(value) };
    any.downcast::<NSString>().ok().map(|s| s.to_string())
}

fn remote_of(record: &CKRecord) -> Option<RemoteRecord> {
    let kind = RecordKind::from_record_type(&unsafe { record.recordType() }.to_string())?;
    let name = unsafe { record.recordID().recordName() }.to_string();
    Some(RemoteRecord { kind, name, payload: payload_of(record)? })
}

fn set_payload(record: &CKRecord, payload: &str) {
    let value = NSString::from_str(payload);
    unsafe { record.setObject_forKey(Some(ProtocolObject::from_ref(&*value)), &NSString::from_str(sync::PAYLOAD_FIELD)) };
}

// ── Delegate ─────────────────────────────────────────────────────────────────

struct DelegateIvars {
    _unit: (),
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "KudzuSyncEngineDelegate"]
    #[ivars = DelegateIvars]
    struct SyncDelegate;

    unsafe impl NSObjectProtocol for SyncDelegate {}

    unsafe impl CKSyncEngineDelegate for SyncDelegate {
        #[unsafe(method(syncEngine:handleEvent:))]
        fn sync_engine_handle_event(&self, _engine: &CKSyncEngine, event: &CKSyncEngineEvent) {
            handle_event(event);
        }

        #[unsafe(method_id(syncEngine:nextRecordZoneChangeBatchForContext:))]
        fn sync_engine_next_batch(
            &self,
            engine: &CKSyncEngine,
            _context: &CKSyncEngineSendChangesContext,
        ) -> Option<Retained<CKSyncEngineRecordZoneChangeBatch>> {
            next_batch(engine)
        }
    }
);

impl SyncDelegate {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateIvars { _unit: () });
        unsafe { msg_send![super(this), init] }
    }
}

// ── Lifecycle ────────────────────────────────────────────────────────────────

pub(super) fn start() {
    let enabled = core().store.lock().settings.icloud_sync;
    if !enabled {
        stop();
        publish(SyncStatus::Off);
        return;
    }
    if STATE.lock().is_some() {
        sync_now();
        return;
    }
    if !entitled() {
        log::info!("iCloud sync unavailable: not signed with the {CONTAINER_ID} entitlement");
        publish(SyncStatus::Unavailable { reason: "This copy of Kudzu isn't signed for iCloud. Release builds sync; development builds stay on this Mac.".into() });
        return;
    }
    publish(SyncStatus::Starting);

    let dir = sync_dir();
    let container = unsafe { CKContainer::containerWithIdentifier(&NSString::from_str(CONTAINER_ID)) };
    let database = unsafe { container.privateCloudDatabase() };
    let saved_state = load_engine_state(&dir);
    let first_launch = saved_state.is_none();
    let delegate = SyncDelegate::new();
    let engine = unsafe {
        let config = CKSyncEngineConfiguration::initWithDatabase_stateSerialization_delegate(
            CKSyncEngineConfiguration::alloc(),
            &database,
            saved_state.as_deref(),
            ProtocolObject::from_ref(&*delegate),
        );
        CKSyncEngine::initWithConfiguration(CKSyncEngine::alloc(), &config)
    };
    let index = load_index(&dir);
    log::info!("iCloud sync starting ({} records known, first launch: {first_launch})", index.records.len());
    *STATE.lock() = Some(Running { engine: engine.clone(), _delegate: delegate, records: HashMap::new(), index, dir });

    if first_launch {
        let zone = unsafe { CKRecordZone::initWithZoneName(CKRecordZone::alloc(), &NSString::from_str(sync::ZONE_NAME)) };
        let save = unsafe { CKSyncEnginePendingZoneSave::initWithZone(CKSyncEnginePendingZoneSave::alloc(), &zone) };
        let changes = NSArray::from_retained_slice(&[Retained::into_super(save)]);
        unsafe { engine.state().addPendingDatabaseChanges(&changes) };
    }

    // Account status for the UI; the engine itself waits for an account on its own.
    let block = RcBlock::new(move |status: objc2_cloud_kit::CKAccountStatus, _err: *mut NSError| {
        use objc2_cloud_kit::CKAccountStatus as S;
        let next = match status {
            S::Available => return,
            S::NoAccount => SyncStatus::NoAccount { reason: "Sign in to iCloud in System Settings to sync your snippets.".into() },
            S::Restricted => SyncStatus::NoAccount { reason: "iCloud is restricted on this Mac.".into() },
            S::TemporarilyUnavailable => SyncStatus::NoAccount { reason: "iCloud is temporarily unavailable. Kudzu will try again.".into() },
            _ => return,
        };
        publish(next);
    });
    unsafe { container.accountStatusWithCompletionHandler(&block) };

    sync_now();
    spawn_timers();
}

pub(super) fn stop() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    let running = STATE.lock().take();
    if let Some(r) = running {
        unsafe { r.engine.cancelOperationsWithCompletionHandler(None) };
    }
}

fn spawn_timers() {
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        let mut since_usage = Duration::ZERO;
        loop {
            std::thread::sleep(FETCH_INTERVAL);
            if GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }
            since_usage += FETCH_INTERVAL;
            if since_usage >= USAGE_INTERVAL {
                since_usage = Duration::ZERO;
                local_changed(true);
            }
            sync_now();
        }
    });
}

pub(super) fn sync_now() {
    if let Some(engine) = engine() {
        unsafe {
            engine.fetchChangesWithCompletionHandler(None);
            engine.sendChangesWithCompletionHandler(None);
        }
    }
}

/// Queues local edits for upload. Waits for the first full fetch on this Mac, so a new Mac
/// doesn't upload its starter snippets before seeing (and merging with) what's in iCloud.
pub(super) fn local_changed(include_usage: bool) {
    let core = core();
    let (changes, engine) = {
        let store = core.store.lock();
        let state = STATE.lock();
        let Some(running) = state.as_ref() else { return };
        if !running.index.initial_merge_done {
            return;
        }
        let mut changes = sync::local_changes(&store.library, &running.index, &store.device_id);
        if !include_usage {
            let own = sync::usage_name(&store.device_id);
            changes.saves.retain(|n| *n != own);
        }
        (changes, running.engine.clone())
    };
    if changes.is_empty() {
        return;
    }
    let mut pending: Vec<Retained<CKSyncEnginePendingRecordZoneChange>> = Vec::new();
    for (names, kind) in [
        (&changes.saves, CKSyncEnginePendingRecordZoneChangeType::SaveRecord),
        (&changes.deletes, CKSyncEnginePendingRecordZoneChangeType::DeleteRecord),
    ] {
        for name in names {
            pending.push(unsafe {
                CKSyncEnginePendingRecordZoneChange::initWithRecordID_type(
                    CKSyncEnginePendingRecordZoneChange::alloc(),
                    &record_id(name),
                    kind,
                )
            });
        }
    }
    unsafe { engine.state().addPendingRecordZoneChanges(&NSArray::from_retained_slice(&pending)) };
}

// ── Sending ──────────────────────────────────────────────────────────────────

fn next_batch(engine: &CKSyncEngine) -> Option<Retained<CKSyncEngineRecordZoneChangeBatch>> {
    let pending = unsafe { engine.state().pendingRecordZoneChanges() };
    if pending.count() == 0 {
        return None;
    }
    let core = core();
    let store = core.store.lock();
    let mut state = STATE.lock();
    let running = state.as_mut()?;
    if running.index.blocked_by_newer_format() {
        return None;
    }
    // Records built for this batch; kept alive until the batch has taken them.
    let built: RefCell<Vec<(String, Retained<CKRecord>)>> = RefCell::new(Vec::new());
    let provider = StackBlock::new(|rid: NonNull<CKRecordID>| -> *mut CKRecord {
        let rid = unsafe { rid.as_ref() };
        let name = unsafe { rid.recordName() }.to_string();
        let Some(rec) = sync::encode(&name, &store.library, &running.index, &store.device_id) else {
            return std::ptr::null_mut(); // gone locally: the engine drops the pending save
        };
        let record = running.records.get(&name).cloned().unwrap_or_else(|| unsafe {
            CKRecord::initWithRecordType_recordID(CKRecord::alloc(), &NSString::from_str(rec.kind.record_type()), rid)
        });
        set_payload(&record, &rec.payload);
        let ptr = Retained::as_ptr(&record) as *mut CKRecord;
        built.borrow_mut().push((name, record));
        ptr
    });
    let batch = unsafe {
        CKSyncEngineRecordZoneChangeBatch::initWithPendingChanges_recordProvider(
            CKSyncEngineRecordZoneChangeBatch::alloc(),
            &pending,
            &provider,
        )
    };
    for (name, record) in built.into_inner() {
        running.records.insert(name, record);
    }
    batch
}

// ── Events ───────────────────────────────────────────────────────────────────

fn handle_event(event: &CKSyncEngineEvent) {
    let kind = unsafe { event.r#type() };
    match kind {
        CKSyncEngineEventType::StateUpdate => {
            let serialization = unsafe { event.stateUpdateEvent().stateSerialization() };
            if let Some(dir) = STATE.lock().as_ref().map(|r| r.dir.clone()) {
                save_engine_state(&dir, &serialization);
            }
        }
        CKSyncEngineEventType::AccountChange => {
            let change = unsafe { event.accountChangeEvent().changeType() };
            log::info!("sync: iCloud account change {:?}", change.0);
            if change == CKSyncEngineAccountChangeType::SignIn {
                publish(SyncStatus::Syncing);
                return;
            }
            // Signed out or switched accounts: forget what the old account held. Local
            // snippets stay; with a new account they upload there after the first fetch.
            if let Some(running) = STATE.lock().as_mut() {
                let dir = running.dir.clone();
                reset_server_knowledge(&dir, running);
            }
            if change == CKSyncEngineAccountChangeType::SignOut {
                publish(SyncStatus::NoAccount { reason: "Signed out of iCloud. Your snippets stay on this Mac.".into() });
            }
        }
        CKSyncEngineEventType::FetchedDatabaseChanges => {
            let deletions = unsafe { event.fetchedDatabaseChangesEvent().deletions() };
            let ours = deletions.iter().any(|d| unsafe { d.zoneID().zoneName() }.to_string() == sync::ZONE_NAME);
            if ours {
                // The zone was deleted (iCloud data reset from another device or Settings).
                // Keep local data and recreate the zone; everything uploads again.
                let engine = {
                    let mut state = STATE.lock();
                    let Some(running) = state.as_mut() else { return };
                    let dir = running.dir.clone();
                    reset_server_knowledge(&dir, running);
                    running.index.initial_merge_done = true;
                    running.engine.clone()
                };
                let zone = unsafe { CKRecordZone::initWithZoneName(CKRecordZone::alloc(), &NSString::from_str(sync::ZONE_NAME)) };
                let save = unsafe { CKSyncEnginePendingZoneSave::initWithZone(CKSyncEnginePendingZoneSave::alloc(), &zone) };
                let changes: Retained<NSArray<CKSyncEnginePendingDatabaseChange>> = NSArray::from_retained_slice(&[Retained::into_super(save)]);
                unsafe { engine.state().addPendingDatabaseChanges(&changes) };
                local_changed(true);
            }
        }
        CKSyncEngineEventType::FetchedRecordZoneChanges => {
            // The engine fetches every zone in the private database, including the Swift app's
            // SwiftData zone in the same container; only Kudzu's zone is ours to read.
            let e = unsafe { event.fetchedRecordZoneChangesEvent() };
            let modified = unsafe { e.modifications() };
            let deleted = unsafe { e.deletions() };
            let records: Vec<Retained<CKRecord>> = modified.iter().filter(|r| in_our_zone(&*unsafe { r.recordID() })).collect();
            let deleted: Vec<String> = deleted
                .iter()
                .filter(|d| in_our_zone(&*unsafe { d.recordID() }))
                .map(|d| unsafe { d.recordID().recordName() }.to_string())
                .collect();
            if !records.is_empty() || !deleted.is_empty() {
                apply(&records, &deleted);
            }
        }
        CKSyncEngineEventType::SentRecordZoneChanges => sent(event),
        CKSyncEngineEventType::WillFetchChanges | CKSyncEngineEventType::WillSendChanges => {
            if !matches!(*STATUS.lock(), SyncStatus::NeedsUpdate) {
                publish(SyncStatus::Syncing);
            }
        }
        CKSyncEngineEventType::DidFetchChanges => {
            let merged = {
                let core = core();
                let mut store = core.store.lock();
                let mut state = STATE.lock();
                let Some(running) = state.as_mut() else { return };
                let first = !running.index.initial_merge_done;
                let changed = sync::initial_merge(&mut store.library, &mut running.index);
                save_index(&running.dir, &running.index);
                (first, changed)
            };
            if merged.1 {
                core().library_changed(); // also queues the merge's deletions
            } else if merged.0 {
                local_changed(true); // first fetch done: now upload what's only here
            }
            settle();
        }
        CKSyncEngineEventType::DidSendChanges if !LAST_SEND_FAILED.load(Ordering::SeqCst) => settle(),
        _ => {}
    }
}

/// After a fetch or send: "up to date", unless blocked by a newer format.
fn settle() {
    let blocked = STATE.lock().as_ref().is_some_and(|r| r.index.blocked_by_newer_format());
    if blocked {
        publish(SyncStatus::NeedsUpdate);
        // Look for the update that would let this Mac sync again.
        std::thread::spawn(|| { let _ = crate::backend::updater::check(true); });
    } else {
        publish(SyncStatus::UpToDate { last_sync: chrono::Utc::now().to_rfc3339() });
    }
}

/// Applies server records to the library and index.
fn apply(records: &[Retained<CKRecord>], deleted: &[String]) {
    let remote: Vec<RemoteRecord> = records.iter().filter_map(|r| remote_of(r)).collect();
    let changed = {
        let core = core();
        let mut store = core.store.lock();
        let device_id = store.device_id.clone();
        let mut state = STATE.lock();
        let Some(running) = state.as_mut() else { return };
        for r in records {
            let name = unsafe { r.recordID().recordName() }.to_string();
            running.records.insert(name, r.clone());
        }
        for name in deleted {
            running.records.remove(name);
        }
        let applied = sync::apply_remote(&mut store.library, &mut running.index, &remote, deleted, &device_id);
        log::info!("sync: received {} records, {} deletions (library changed: {})", remote.len(), deleted.len(), applied.library_changed);
        save_index(&running.dir, &running.index);
        applied.library_changed
    };
    if changed {
        core().library_changed();
    }
}

fn sent(event: &CKSyncEngineEvent) {
    let e = unsafe { event.sentRecordZoneChangesEvent() };
    let saved = unsafe { e.savedRecords() };
    let deleted = unsafe { e.deletedRecordIDs() };
    let failed = unsafe { e.failedRecordSaves() };

    log::info!("sync: sent {} saves, {} deletes, {} failed", saved.count(), deleted.count(), failed.count());
    let mut resend: Vec<String> = Vec::new();
    let mut conflicts: Vec<Retained<CKRecord>> = Vec::new();
    let mut zone_missing = false;
    let mut problem: Option<String> = None;
    {
        let mut state = STATE.lock();
        let Some(running) = state.as_mut() else { return };
        for record in saved.iter() {
            if let Some(remote) = remote_of(&record) {
                sync::record_saved(&mut running.index, &remote);
                running.records.insert(remote.name, record.clone());
            }
        }
        for rid in deleted.iter() {
            let name = unsafe { rid.recordName() }.to_string();
            sync::record_deleted(&mut running.index, &name);
            running.records.remove(&name);
        }
        for failure in failed.iter() {
            let record = unsafe { failure.record() };
            let error = unsafe { failure.error() };
            let name = unsafe { record.recordID().recordName() }.to_string();
            match CKErrorCode(error.code()) {
                CKErrorCode::ServerRecordChanged => {
                    // Someone else saved first: take their record (and its change tag), then
                    // let the merge rules decide; if ours still wins it's resent on top.
                    if let Some(server) = server_record(&error) {
                        conflicts.push(server);
                    }
                    resend.push(name);
                }
                CKErrorCode::ZoneNotFound => {
                    zone_missing = true;
                    resend.push(name);
                }
                CKErrorCode::UnknownItem => {
                    // Our cached copy refers to a record the server no longer has: start fresh.
                    running.records.remove(&name);
                    resend.push(name);
                }
                code => {
                    let message = error.localizedDescription().to_string();
                    log::warn!("sync: couldn't save {name}: {message}");
                    // The engine retries transient failures itself; surface only the rest.
                    let transient = [
                        CKErrorCode::NetworkUnavailable,
                        CKErrorCode::NetworkFailure,
                        CKErrorCode::ServiceUnavailable,
                        CKErrorCode::RequestRateLimited,
                        CKErrorCode::ZoneBusy,
                    ];
                    if !transient.contains(&code) {
                        problem = Some(if code == CKErrorCode::QuotaExceeded {
                            "Your iCloud storage is full, so new changes aren't syncing.".into()
                        } else {
                            message
                        });
                    }
                }
            }
        }
        save_index(&running.dir, &running.index);
    }
    if !conflicts.is_empty() {
        apply(&conflicts, &[]);
    }
    if zone_missing
        && let Some(engine) = engine() {
            let zone = unsafe { CKRecordZone::initWithZoneName(CKRecordZone::alloc(), &NSString::from_str(sync::ZONE_NAME)) };
            let save = unsafe { CKSyncEnginePendingZoneSave::initWithZone(CKSyncEnginePendingZoneSave::alloc(), &zone) };
            let changes: Retained<NSArray<CKSyncEnginePendingDatabaseChange>> = NSArray::from_retained_slice(&[Retained::into_super(save)]);
            unsafe { engine.state().addPendingDatabaseChanges(&changes) };
        }
    if !resend.is_empty() {
        local_changed(true);
    }
    LAST_SEND_FAILED.store(problem.is_some(), Ordering::SeqCst);
    if let Some(message) = problem {
        publish(SyncStatus::Error { message });
    }
}

fn server_record(error: &NSError) -> Option<Retained<CKRecord>> {
    let info = error.userInfo();
    let value = info.objectForKey(unsafe { CKRecordChangedErrorServerRecordKey })?;
    value.downcast::<CKRecord>().ok()
}
