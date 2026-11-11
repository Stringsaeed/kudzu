//! Cloud transport only. The sync service owns merges, status, and generation fencing.
//!
//! S37 implements this on `CKSyncEngine` and must not merge the library itself. Names come from
//! [`kudzu_core::sync`](kudzu_core::sync): `CONTAINER_ID` (`iCloud.sh.saeed.kudzu`), `ZONE_NAME`
//! (`Kudzu`), and `RecordKind`. Kudzu Next never reaches `start` (the service refuses Dev).
//! The service also never calls the port after `start` returns [`PortError::Unavailable`].
//!
//! Port methods run on the sync worker. Events may be delivered synchronously from inside
//! `start`, `fetch`, or `send`; the service only enqueues them. Callbacks carry the generation
//! passed to `start`. The service drops a stale one. No events are delivered after `stop`.
//!
//! Only the Kudzu zone is reported, never another zone in the container. `full_fetch` is set
//! on `DidFetchChanges`. `start(checkpoint: None)` means there is no saved state, so the adapter
//! queues the zone save. A later start builds a new engine.
//!
//! `zone_deleted` comes from fetched database changes filtered to the Kudzu zone. `zone_missing`
//! comes from a send's `ZoneNotFound`. In both cases the adapter queues the zone save. The
//! service re-derives resends from its index; the adapter never re-adds `requeue` names itself.
//!
//! `CloudSend.conflicts` is only `ServerRecordChanged`. The adapter replaces its cached record
//! (change tag) with the server copy before reporting. A save's `UnknownItem` drops the cached
//! record and lists the name in `requeue`. A delete's `UnknownItem` is reported in `deleted`.
//!
//! Transient codes stay inside the adapter (`NetworkUnavailable`, `NetworkFailure`,
//! `ServiceUnavailable`, `RequestRateLimited`, `ZoneBusy`). `QuotaExceeded` maps to
//! [`CloudSendFailure::QuotaExceeded`]. Everything else is [`CloudSendFailure::Permanent`].
//! `Err` from `fetch` or `send` means the request could not be issued at all. Never forward
//! `localizedDescription`.
//!
//! # Contract
//!
//! These decisions bind the sync service and the S37 adapter.
//!
//! 1. **Conflicts.** When the adapter reports a name in `conflicts`, it first drops that name's
//!    pending payload. The service merges the server copy and resends only if its result still
//!    differs. The adapter never resends a losing payload by itself.
//! 2. **Failures.** [`CloudSend::failure`] (`QuotaExceeded` or `Permanent`) applies to the records
//!    of that send that are in neither `saved` nor `deleted`. The adapter drops those records from
//!    its pending set, as `CKSyncEngine` does. On any failure the service clears those names from
//!    its in-flight set and queues a resend on its next push. Sends are idempotent upserts. Status
//!    shows the failure sentence until a later send succeeds.
//! 3. **Deletes of missing records.** A delete whose record is already gone on the server
//!    (`unknownItem`) is reported in `deleted`, so the service can clear it.
//! 4. **Restored engine state.** `CKSyncEngine` state restored at `start` may contain pending
//!    record changes whose payloads the adapter no longer has. The adapter drops restored pending
//!    record changes. The service re-derives everything not acknowledged from its index after start.
//! 5. **Fetch.** Every `fetch` call is answered by at least one `full_fetch: true` batch when a
//!    fetch completes, even when it coalesces with one already running. A fetch that fails
//!    transiently produces no batch and no `failed`. Status follows the Tauri app: "Syncing…" is
//!    shown while a fetch or send is outstanding. If no `full_fetch` or `sent` arrives by the next
//!    tick, the service returns to its last settled status (`UpToDate` keeps its earlier time).
//!    Before the first full fetch it stays "Syncing…". A partial fetch does not set
//!    `initial_merge_done`. Zone deletion is not a fetch: it marks the index past the first fetch
//!    so the kept library is re-sent.
//! 6. **Unknown records.** The adapter drops records in the Kudzu zone whose type isn't a
//!    `RecordKind` or whose name doesn't parse, and logs only a count.
//! 7. **Volume.** `send` may carry thousands of records. The adapter hands them to `CKSyncEngine`'s
//!    pending set, which batches them itself. The service coalesces state: per worker turn it keeps
//!    only the newest state bytes and writes them once at flush.
//! 8. **`failed(Unavailable)` mid-run** is allowed. The process lost its entitlement or iCloud
//!    became restricted. The service treats `Unavailable` as permanent for the process lifetime:
//!    it stops the port, shows Unavailable, and never calls the port again until relaunch. Turning
//!    sync off and on does not retry.
//!
//! Account status after every successful `start`, and on every later change:
//! `Available`, `NoAccount`, `Restricted`, and `TemporarilyUnavailable` are reported as
//! [`CloudAvailability`]. `CouldNotDetermine` is reported too; the service keeps its current
//! status (Tauri ignored that case). Sign-in is [`AccountChange::SignIn`] plus availability.
//! Sign-out is [`AccountChange::SignOut`]. An account switch is [`AccountChange::Switch`].

use crate::error::PortError;
use kudzu_core::sync::RemoteRecord;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudAvailability {
    Available,
    NoAccount,
    Restricted,
    TemporarilyUnavailable,
    /// `CKAccountStatus` could not be read. The service keeps whatever it is already showing.
    CouldNotDetermine,
    /// The process is not entitled. `start` returns [`PortError::Unavailable`] for this too.
    Unavailable,
}

/// Why the iCloud account identity changed. Availability is a separate callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountChange {
    /// The same account became available. The service does not drop server knowledge.
    SignIn,
    /// The user signed out. The service drops server knowledge and shows the signed-out sentence.
    SignOut,
    /// A different account is signed in. The service drops server knowledge and keeps syncing.
    Switch,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CloudBatch {
    pub records: Vec<RemoteRecord>,
    pub deleted: Vec<String>,
    /// The fetch finished, including a fetch that changed nothing.
    pub full_fetch: bool,
}

/// What an attempt to send reported. Transient CloudKit failures stay out of this;
/// the engine retries those itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CloudSend {
    /// Server-returned payloads, not the payloads the service submitted.
    pub saved: Vec<RemoteRecord>,
    pub deleted: Vec<String>,
    /// `serverRecordChanged`: the server copies to merge, then the saves are requeued.
    pub conflicts: Vec<RemoteRecord>,
    pub requeue: Vec<String>,
    pub zone_missing: bool,
    pub failure: Option<CloudSendFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudSendFailure {
    QuotaExceeded,
    Permanent,
}

/// Callbacks are typed inbound methods, not a UI notification channel. Never hold adapter locks.
pub trait CloudEvents: Send + Sync {
    fn fetched(&self, generation: u64, batch: CloudBatch);
    fn sent(&self, generation: u64, report: CloudSend);
    fn account_changed(&self, generation: u64, change: AccountChange);
    fn availability(&self, generation: u64, availability: CloudAvailability);
    fn failed(&self, generation: u64, error: PortError);
    /// The Kudzu zone was deleted. The adapter has already queued its recreation.
    fn zone_deleted(&self, generation: u64);
    /// CKSyncEngine state (NSKeyedArchiver bytes), in order with the other events.
    /// Empty bytes are never sent. The service writes them only after earlier merges are durable.
    fn state(&self, generation: u64, bytes: Vec<u8>);
}

pub trait CloudStore: Send + Sync {
    /// `checkpoint == None` means no saved state; queue the zone save. `Some` restores it.
    fn start(
        &self,
        generation: u64,
        checkpoint: Option<Vec<u8>>,
        events: Arc<dyn CloudEvents>,
    ) -> Result<(), PortError>;
    /// Safe without `start`, idempotent, and must not touch CloudKit when unentitled.
    /// No events follow `stop`.
    fn stop(&self);
    fn fetch(&self, generation: u64) -> Result<(), PortError>;
    /// Upserts the adapter's pending set by record name. The latest payload wins, a delete
    /// replaces a pending save, and repeating a payload is idempotent. An empty batch means
    /// "send what is already pending"; it does not clear that set.
    fn send(&self, generation: u64, batch: CloudBatch) -> Result<(), PortError>;
}
