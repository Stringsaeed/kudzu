//! Cloud transport only. The sync service owns merges, status, and generation fencing.
//!
//! S37 implements this on `CKSyncEngine` and must not merge the library itself.
//! Container `iCloud.sh.saeed.kudzu`. Zone and record types come from `kudzu_core::sync`
//! (`ZONE_NAME`, `RecordKind`). Kudzu Next never reaches `start` (the service refuses Dev).
//!
//! Callbacks carry the generation passed to `start`. The service drops a stale one.
//! `checkpoint` returns the engine serialization (`engine-state.data`); `Err` leaves the
//! previous bytes in place. `start` after `stop` builds a new engine. Zone recreation stays
//! in the adapter: queue the zone save, then call `zone_deleted`.
//!
//! Account events: sign-out and an account switch call `account_changed` (the service drops
//! server knowledge and restarts). Sign-in calls `availability(Available)` only.
use crate::error::PortError;
use kudzu_core::sync::RemoteRecord;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudAvailability {
    Available,
    NoAccount,
    Restricted,
    TemporarilyUnavailable,
    /// The process is not entitled. `start` returns [`PortError::Unavailable`] for this too.
    Unavailable,
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
    fn account_changed(&self, generation: u64);
    fn availability(&self, generation: u64, availability: CloudAvailability);
    fn failed(&self, generation: u64, error: PortError);
    /// The Kudzu zone was deleted. The adapter has already queued its recreation.
    fn zone_deleted(&self, generation: u64);
}

pub trait CloudStore: Send + Sync {
    fn start(
        &self,
        generation: u64,
        checkpoint: Option<Vec<u8>>,
        events: Arc<dyn CloudEvents>,
    ) -> Result<(), PortError>;
    fn stop(&self);
    fn fetch(&self, generation: u64) -> Result<(), PortError>;
    fn send(&self, generation: u64, batch: CloudBatch) -> Result<(), PortError>;
    /// Last engine serialization. `Err` means keep the previous checkpoint.
    fn checkpoint(&self) -> Result<Vec<u8>, PortError>;
}
