//! Scriptable cloud callbacks, including callbacks from obsolete generations.
use crate::{
    error::PortError,
    ports::{CloudAvailability, CloudBatch, CloudEvents, CloudStore},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
#[derive(Default)]
pub struct CloudState {
    pub active: Option<u64>,
    pub fetches: Vec<u64>,
    pub sends: Vec<(u64, CloudBatch)>,
    pub checkpoint: Vec<u8>,
    pub failure: Option<PortError>,
}
#[derive(Default)]
pub struct FakeCloud {
    pub state: Mutex<CloudState>,
    callbacks: Mutex<BTreeMap<u64, Arc<dyn CloudEvents>>>,
}
impl FakeCloud {
    pub fn events(&self, generation: u64) -> Option<Arc<dyn CloudEvents>> {
        self.callbacks.lock().unwrap().get(&generation).cloned()
    }
    pub fn deliver(&self, generation: u64, batch: CloudBatch) {
        if let Some(events) = self.events(generation) {
            events.fetched(generation, batch);
        }
    }
}
impl CloudStore for FakeCloud {
    fn start(
        &self,
        generation: u64,
        checkpoint: Option<Vec<u8>>,
        events: Arc<dyn CloudEvents>,
    ) -> Result<(), PortError> {
        let mut state = self.state.lock().unwrap();
        if let Some(e) = state.failure {
            return Err(e);
        }
        state.active = Some(generation);
        state.checkpoint = checkpoint.unwrap_or_default();
        self.callbacks.lock().unwrap().insert(generation, events);
        Ok(())
    }
    fn stop(&self) {
        self.state.lock().unwrap().active = None;
    }
    fn fetch(&self, generation: u64) -> Result<(), PortError> {
        let mut state = self.state.lock().unwrap();
        if let Some(e) = state.failure {
            return Err(e);
        }
        state.fetches.push(generation);
        Ok(())
    }
    fn send(&self, generation: u64, batch: CloudBatch) -> Result<(), PortError> {
        let mut state = self.state.lock().unwrap();
        if let Some(e) = state.failure {
            return Err(e);
        }
        state.sends.push((generation, batch));
        Ok(())
    }
    fn checkpoint(&self) -> Result<Vec<u8>, PortError> {
        let state = self.state.lock().unwrap();
        if let Some(e) = state.failure {
            return Err(e);
        }
        Ok(state.checkpoint.clone())
    }
}
#[derive(Default)]
pub struct FakeCloudEvents {
    pub fetched: Mutex<Vec<(u64, CloudBatch)>>,
    pub sent: Mutex<Vec<(u64, CloudBatch)>>,
    pub accounts: Mutex<Vec<u64>>,
    pub availability: Mutex<Vec<(u64, CloudAvailability)>>,
    pub failures: Mutex<Vec<(u64, PortError)>>,
}
impl CloudEvents for FakeCloudEvents {
    fn fetched(&self, generation: u64, batch: CloudBatch) {
        self.fetched.lock().unwrap().push((generation, batch));
    }
    fn sent(&self, generation: u64, batch: CloudBatch) {
        self.sent.lock().unwrap().push((generation, batch));
    }
    fn account_changed(&self, generation: u64) {
        self.accounts.lock().unwrap().push(generation);
    }
    fn availability(&self, generation: u64, state: CloudAvailability) {
        self.availability.lock().unwrap().push((generation, state));
    }
    fn failed(&self, generation: u64, error: PortError) {
        self.failures.lock().unwrap().push((generation, error));
    }
}
