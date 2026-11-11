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
//! (change tag) with the server copy before reporting. `UnknownItem` drops the cached record and
//! lists the name in `requeue`.
//!
//! Transient codes stay inside the adapter (`NetworkUnavailable`, `NetworkFailure`,
//! `ServiceUnavailable`, `RequestRateLimited`, `ZoneBusy`). `QuotaExceeded` maps to
//! [`CloudSendFailure::QuotaExceeded`]. Everything else is [`CloudSendFailure::Permanent`].
//! `Err` from `fetch` or `send` means the request could not be issued at all. Never forward
//! `localizedDescription`.
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
