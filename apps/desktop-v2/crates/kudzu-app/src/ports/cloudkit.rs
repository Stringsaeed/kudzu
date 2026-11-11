//! Cloud transport only. The sync service owns merges, status, and generation fencing.
use crate::error::PortError;
use kudzu_core::sync::RemoteRecord;
use std::sync::Arc;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudAvailability {
    Available,
    NoAccount,
    Unavailable,
}
#[derive(Clone, Default)]
pub struct CloudBatch {
    pub records: Vec<RemoteRecord>,
    pub deleted: Vec<String>,
    pub full_fetch: bool,
}
/// Callbacks are typed inbound methods, not a UI notification channel. Never hold adapter locks.
pub trait CloudEvents: Send + Sync {
    fn fetched(&self, generation: u64, batch: CloudBatch);
    fn sent(&self, generation: u64, batch: CloudBatch);
    fn account_changed(&self, generation: u64);
    fn availability(&self, generation: u64, availability: CloudAvailability);
    fn failed(&self, generation: u64, error: PortError);
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
    fn checkpoint(&self) -> Result<Vec<u8>, PortError>;
}
