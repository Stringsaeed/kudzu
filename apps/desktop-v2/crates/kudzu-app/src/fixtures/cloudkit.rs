//! Scriptable cloud callbacks, including callbacks from obsolete generations.
//!
//! `failure` fails `fetch` and `send` together. `start_failure`, `fetch_failure`, and
//! `send_failure` are the per-call switches. `script` runs synchronously at the end of
//! `start`, `fetch`, or `send`, before that call returns; the service must only enqueue.
//! After `stop`, `fetch` and `send` return [`PortError::Cancelled`] and `deliver` is a no-op
//! until the next `start`. A generation registered before `stop` can be delivered again once
//! a later `start` is active, so fencing tests can inject a stale event.

use crate::{
    error::PortError,
    ports::{AccountChange, CloudAvailability, CloudBatch, CloudEvents, CloudSend, CloudStore},
};
use kudzu_core::sync::RemoteRecord;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingRecord {
    Save(RemoteRecord),
    Delete,
}

/// Events injected from inside `start`, `fetch`, or `send`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudScript {
    Fetched(CloudBatch),
    Sent(CloudSend),
    State(Vec<u8>),
    Availability(CloudAvailability),
    Account(AccountChange),
    Failed(PortError),
    ZoneDeleted,
}

#[derive(Clone, Default)]
pub struct CloudState {
    pub active: Option<u64>,
    pub fetches: Vec<u64>,
    pub sends: Vec<(u64, CloudBatch)>,
    /// Adapter pending set. Latest payload wins; a delete replaces a save.
    pub pending: BTreeMap<String, PendingRecord>,
    pub starts: Vec<(u64, Option<Vec<u8>>)>,
    /// How many `start` calls passed `None` (no saved state; the adapter queues the zone save).
    pub fresh_starts: u32,
    pub stops: u32,
    pub failure: Option<PortError>,
    pub start_failure: Option<PortError>,
    pub fetch_failure: Option<PortError>,
    pub send_failure: Option<PortError>,
    /// Keeps pending records when a `CloudSend.failure` is reported. The default drops them.
    pub retain_failed: bool,
    pub script: Vec<CloudScript>,
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

    /// Delivers only while a session is active. Dropped after `stop` until the next `start`.
    pub fn deliver(&self, generation: u64, batch: CloudBatch) {
        if self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .active
            .is_none()
        {
            return;
        }
        if let Some(events) = self.events(generation) {
            events.fetched(generation, batch);
        }
    }

    pub fn fail_start(&self, error: Option<PortError>) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .start_failure = error;
    }

    /// Reports the pending set. Every delete is `deleted`, including one whose record
    /// was already missing, and the report is delivered to the service.
    pub fn take_report(&self, generation: u64) -> CloudSend {
        let report = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let pending = std::mem::take(&mut state.pending);
            let mut saved = Vec::new();
            let mut deleted = Vec::new();
            for (name, record) in pending {
                match record {
                    PendingRecord::Save(record) => saved.push(record),
                    PendingRecord::Delete => deleted.push(name),
                }
            }
            CloudSend {
                saved,
                deleted,
                ..CloudSend::default()
            }
        };
        if let Some(events) = self.events(generation) {
            events.sent(generation, report.clone());
        }
        report
    }

    pub fn script(&self, events: Vec<CloudScript>) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .script = events;
    }

    fn drain_script(&self, generation: u64) {
        let script = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            std::mem::take(&mut state.script)
        };
        let Some(events) = self.events(generation) else {
            return;
        };
        for event in script {
            if self
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .active
                .is_none()
            {
                break;
            }
            match event {
                CloudScript::Fetched(batch) => events.fetched(generation, batch),
                CloudScript::Sent(report) => {
                    {
                        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
                        apply_sent(&mut state, &report);
                    }
                    events.sent(generation, report);
                }
                CloudScript::State(bytes) => events.state(generation, bytes),
                CloudScript::Availability(availability) => {
                    events.availability(generation, availability)
                }
                CloudScript::Account(change) => events.account_changed(generation, change),
                CloudScript::Failed(error) => events.failed(generation, error),
                CloudScript::ZoneDeleted => events.zone_deleted(generation),
            }
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
        {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(error) = state.start_failure {
                return Err(error);
            }
            state.active = Some(generation);
            if checkpoint.is_none() {
                state.fresh_starts += 1;
            }
            state.starts.push((generation, checkpoint));
        }
        self.callbacks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(generation, events);
        self.drain_script(generation);
        Ok(())
    }

    fn stop(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.stops += 1;
        state.active = None;
    }

    fn fetch(&self, generation: u64) -> Result<(), PortError> {
        {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if state.active != Some(generation) {
                return Err(PortError::Cancelled);
            }
            if let Some(error) = state.fetch_failure.or(state.failure) {
                return Err(error);
            }
            state.fetches.push(generation);
        }
        self.drain_script(generation);
        Ok(())
    }

    fn send(&self, generation: u64, batch: CloudBatch) -> Result<(), PortError> {
        {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if state.active != Some(generation) {
                return Err(PortError::Cancelled);
            }
            if let Some(error) = state.send_failure.or(state.failure) {
                return Err(error);
            }
            for record in &batch.records {
                state
                    .pending
                    .insert(record.name.clone(), PendingRecord::Save(record.clone()));
            }
            for name in &batch.deleted {
                state.pending.insert(name.clone(), PendingRecord::Delete);
            }
            state.sends.push((generation, batch));
        }
        self.drain_script(generation);
        Ok(())
    }
}

fn apply_sent(state: &mut CloudState, report: &CloudSend) {
    for record in &report.conflicts {
        state.pending.remove(&record.name);
    }
    if report.failure.is_none() || state.retain_failed {
        return;
    }
    let Some((_, batch)) = state.sends.last() else {
        return;
    };
    let batch = batch.clone();
    for record in &batch.records {
        let kept = report.saved.iter().any(|saved| saved.name == record.name)
            || report.deleted.iter().any(|name| name == &record.name);
        if !kept {
            state.pending.remove(&record.name);
        }
    }
    for name in &batch.deleted {
        let kept = report.saved.iter().any(|saved| &saved.name == name)
            || report.deleted.iter().any(|deleted| deleted == name);
        if !kept {
            state.pending.remove(name);
        }
    }
}

#[derive(Default)]
pub struct FakeCloudEvents {
    pub fetched: Mutex<Vec<(u64, CloudBatch)>>,
    pub sent: Mutex<Vec<(u64, CloudSend)>>,
    pub accounts: Mutex<Vec<(u64, AccountChange)>>,
    pub availability: Mutex<Vec<(u64, CloudAvailability)>>,
    pub failures: Mutex<Vec<(u64, PortError)>>,
    pub zones: Mutex<Vec<u64>>,
    pub states: Mutex<Vec<(u64, Vec<u8>)>>,
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

    fn account_changed(&self, generation: u64, change: AccountChange) {
        self.accounts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((generation, change));
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

    fn state(&self, generation: u64, bytes: Vec<u8>) {
        self.states
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((generation, bytes));
    }
}
