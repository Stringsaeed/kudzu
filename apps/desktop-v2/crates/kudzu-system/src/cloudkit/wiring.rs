//! The only CloudStore/CloudEvents wiring. Native translation has no service access.
use super::{
    entitlement,
    native::{Event, Running, Session},
};
use kudzu_app::{
    error::PortError,
    ports::{AccountChange, CloudAvailability, CloudBatch, CloudEvents, CloudStore},
};
use kudzu_core::sync::CONTAINER_ID;
use objc2_cloud_kit::{CKAccountStatus, CKSyncEngineAccountChangeType};
use std::sync::{
    mpsc::{Receiver, RecvTimeoutError},
    Arc, Mutex, PoisonError,
};
use std::time::Duration;

/// Construct once in S40 and pass as `Arc<dyn CloudStore>` to the sync service.
/// Construction itself does not access CloudKit or check the signed identity.
#[derive(Default)]
pub struct CloudKit {
    slot: Mutex<Slot>,
}

#[derive(Default)]
enum Slot {
    #[default]
    Off,
    Starting(Arc<Session>),
    Running(Arc<Running>),
    Stopping,
    Retiring(Receiver<()>),
}

impl CloudKit {
    pub fn new() -> Self {
        Self::default()
    }

    fn start_engine(
        &self,
        generation: u64,
        checkpoint: Option<Vec<u8>>,
        sink: impl Fn(u64, Event) + Send + Sync + 'static,
    ) -> Result<(), PortError> {
        entitlement::require(entitlement::entitled(CONTAINER_ID), || {
            let session = Arc::new(Session::new(generation, sink));
            let retired = {
                let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
                if !matches!(*slot, Slot::Off | Slot::Retiring(_)) {
                    return Err(PortError::Cancelled);
                }
                match std::mem::replace(&mut *slot, Slot::Starting(session.clone())) {
                    Slot::Retiring(receiver) => Some(receiver),
                    _ => None,
                }
            };
            if let Some(receiver) = retired {
                // CK cancellation is asynchronous. Never create two live engines for the
                // same database. Keep the retirement on timeout so a retry can finish it.
                match receiver.recv_timeout(Duration::from_secs(30)) {
                    Ok(()) => (),
                    Err(error) => {
                        session.delivery.stop();
                        *self.slot.lock().unwrap_or_else(PoisonError::into_inner) =
                            Slot::Retiring(receiver);
                        return Err(match error {
                            RecvTimeoutError::Timeout => PortError::TimedOut,
                            RecvTimeoutError::Disconnected => PortError::Unavailable,
                        });
                    }
                }
            }
            let running = if session.delivery.is_active() {
                Ok(Arc::new(Running::start(checkpoint, session.clone())))
            } else {
                Err(PortError::Cancelled)
            };
            match running {
                Ok(running) => {
                    let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
                    if matches!(&*slot, Slot::Starting(current) if Arc::ptr_eq(current, &session)) {
                        *slot = Slot::Running(running);
                        return Ok(());
                    }
                    drop(slot);
                    let receiver = running.cancel();
                    *self.slot.lock().unwrap_or_else(PoisonError::into_inner) =
                        Slot::Retiring(receiver);
                    Err(PortError::Cancelled)
                }
                Err(error) => {
                    session.delivery.stop();
                    *self.slot.lock().unwrap_or_else(PoisonError::into_inner) = Slot::Off;
                    Err(error)
                }
            }
        })
    }

    fn running(&self, generation: u64) -> Result<Arc<Running>, PortError> {
        let slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        match &*slot {
            Slot::Running(running) if running.session.delivery.generation() == generation => {
                Ok(running.clone())
            }
            _ => Err(PortError::Cancelled),
        }
    }

    fn stop_engine(&self) {
        let running = {
            let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
            if matches!(*slot, Slot::Starting(_)) {
                let Slot::Starting(session) = std::mem::replace(&mut *slot, Slot::Stopping) else {
                    unreachable!()
                };
                drop(slot);
                session.delivery.stop();
                return;
            }
            if !matches!(*slot, Slot::Running(_)) {
                return;
            }
            match std::mem::replace(&mut *slot, Slot::Stopping) {
                Slot::Running(running) => running,
                _ => unreachable!(),
            }
        };
        let receiver = running.cancel();
        *self.slot.lock().unwrap_or_else(PoisonError::into_inner) = Slot::Retiring(receiver);
    }
}

impl CloudStore for CloudKit {
    fn start(
        &self,
        generation: u64,
        checkpoint: Option<Vec<u8>>,
        events: Arc<dyn CloudEvents>,
    ) -> Result<(), PortError> {
        self.start_engine(generation, checkpoint, move |generation, event| {
            forward(events.as_ref(), generation, event);
        })
    }

    fn stop(&self) {
        self.stop_engine();
    }

    fn fetch(&self, generation: u64) -> Result<(), PortError> {
        let running = self.running(generation)?;
        let _lease = running
            .session
            .delivery
            .enter()
            .ok_or(PortError::Cancelled)?;
        running.fetch();
        Ok(())
    }

    fn send(&self, generation: u64, batch: CloudBatch) -> Result<(), PortError> {
        let running = self.running(generation)?;
        let _lease = running
            .session
            .delivery
            .enter()
            .ok_or(PortError::Cancelled)?;
        running.send(batch);
        Ok(())
    }
}

fn forward(events: &dyn CloudEvents, generation: u64, event: Event) {
    match event {
        Event::Fetched(batch) => events.fetched(generation, batch),
        Event::Sent(report) => events.sent(generation, report),
        Event::State(bytes) => {
            if !bytes.is_empty() {
                events.state(generation, bytes);
            }
        }
        Event::Account(change) => {
            let change = match change {
                CKSyncEngineAccountChangeType::SignIn => AccountChange::SignIn,
                CKSyncEngineAccountChangeType::SignOut => AccountChange::SignOut,
                CKSyncEngineAccountChangeType::SwitchAccounts => AccountChange::Switch,
                _ => return,
            };
            events.account_changed(generation, change);
        }
        Event::Availability(status) => {
            let status = match status {
                CKAccountStatus::Available => CloudAvailability::Available,
                CKAccountStatus::NoAccount => CloudAvailability::NoAccount,
                CKAccountStatus::Restricted => CloudAvailability::Restricted,
                CKAccountStatus::TemporarilyUnavailable => {
                    CloudAvailability::TemporarilyUnavailable
                }
                // Keep the service's current status for an indeterminate or future value.
                _ => CloudAvailability::CouldNotDetermine,
            };
            events.availability(generation, status);
        }
        Event::Failed(error) => events.failed(generation, error),
        Event::ZoneDeleted => events.zone_deleted(generation),
    }
}

impl Drop for CloudKit {
    fn drop(&mut self) {
        self.stop_engine();
    }
}

#[cfg(test)]
mod tests;
