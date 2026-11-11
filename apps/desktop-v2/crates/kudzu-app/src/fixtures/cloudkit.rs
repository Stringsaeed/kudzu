//! Scriptable cloud callbacks, including callbacks from obsolete generations.
use crate::{
    error::PortError,
    ports::{CloudAvailability, CloudBatch, CloudEvents, CloudSend, CloudStore},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError},
};

#[derive(Clone, Default)]
pub struct CloudState {
    pub active: Option<u64>,
    pub fetches: Vec<u64>,
    pub sends: Vec<(u64, CloudBatch)>,
    pub starts: Vec<(u64, Option<Vec<u8>>)>,
    /// Checkpoint bytes last passed to `start`. `checkpoint()` returns these unless
    /// `engine_state` is set.
    pub checkpoint: Vec<u8>,
    /// When set, `checkpoint()` returns this instead of the bytes passed to `start`.
    pub engine_state: Option<Vec<u8>>,
    pub failure: Option<PortError>,
    pub start_failure: Option<PortError>,
    pub checkpoint_failure: Option<PortError>,
}

#[derive(Default)]
pub struct FakeCloud {
    pub state: Mutex<CloudState>,
    callbacks: Mutex<BTreeMap<u64, Arc<dyn CloudEvents>>>,
}

impl FakeCloud {
    pub fn events(&self, generation: u64) -> Option<Arc<dyn CloudEvents>> {
        self.callbacks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&generation)
            .cloned()
    }

    pub fn deliver(&self, generation: u64, batch: CloudBatch) {
        if let Some(events) = self.events(generation) {
            events.fetched(generation, batch);
        }
    }

    pub fn set_engine_state(&self, bytes: impl Into<Vec<u8>>) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .engine_state = Some(bytes.into());
    }

    pub fn fail_start(&self, error: Option<PortError>) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .start_failure = error;
    }

    pub fn fail_checkpoint(&self, error: Option<PortError>) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .checkpoint_failure = error;
    }
}

impl CloudStore for FakeCloud {
    fn start(
        &self,
        generation: u64,
        checkpoint: Option<Vec<u8>>,
        events: Arc<dyn CloudEvents>,
    ) -> Result<(), PortError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(error) = state.start_failure.or(state.failure) {
            return Err(error);
        }
        state.active = Some(generation);
        state.starts.push((generation, checkpoint.clone()));
        state.checkpoint = checkpoint.unwrap_or_default();
        drop(state);
        self.callbacks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(generation, events);
        Ok(())
    }

    fn stop(&self) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .active = None;
    }

    fn fetch(&self, generation: u64) -> Result<(), PortError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(error) = state.failure {
            return Err(error);
        }
        state.fetches.push(generation);
        Ok(())
    }

    fn send(&self, generation: u64, batch: CloudBatch) -> Result<(), PortError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(error) = state.failure {
            return Err(error);
        }
        state.sends.push((generation, batch));
        Ok(())
    }

    fn checkpoint(&self) -> Result<Vec<u8>, PortError> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(error) = state.checkpoint_failure.or(state.failure) {
            return Err(error);
        }
        Ok(state
            .engine_state
            .clone()
            .unwrap_or_else(|| state.checkpoint.clone()))
    }
}

#[derive(Default)]
pub struct FakeCloudEvents {
    pub fetched: Mutex<Vec<(u64, CloudBatch)>>,
    pub sent: Mutex<Vec<(u64, CloudSend)>>,
    pub accounts: Mutex<Vec<u64>>,
    pub availability: Mutex<Vec<(u64, CloudAvailability)>>,
    pub failures: Mutex<Vec<(u64, PortError)>>,
    pub zones: Mutex<Vec<u64>>,
}

impl CloudEvents for FakeCloudEvents {
    fn fetched(&self, generation: u64, batch: CloudBatch) {
        self.fetched
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((generation, batch));
    }

    fn sent(&self, generation: u64, report: CloudSend) {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((generation, report));
    }

    fn account_changed(&self, generation: u64) {
        self.accounts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(generation);
    }

    fn availability(&self, generation: u64, state: CloudAvailability) {
        self.availability
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((generation, state));
    }

    fn failed(&self, generation: u64, error: PortError) {
        self.failures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((generation, error));
    }

    fn zone_deleted(&self, generation: u64) {
        self.zones
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(generation);
    }
}
