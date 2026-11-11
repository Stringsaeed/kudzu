//! One serialised sync turn: merge inside `library.transact`, then the index, then sends.
//! Callbacks from an older generation are ignored. Nothing here runs on the UI thread.
//!
//! Availability other than [`CloudAvailability::Available`] stays on screen across ticks,
//! Sync now, and fetch failures. [`CloudAvailability::CouldNotDetermine`] changes nothing.
//! Sign-out keeps [`SIGNED_OUT`](super::status::SIGNED_OUT) until `Available`.

use super::apply::Flight;
use super::files;
use super::status::{
    self, SyncStatus, DEV_UNAVAILABLE, START_FAILED, TEMPORARILY_UNAVAILABLE, UNENTITLED,
};
use super::{bump, enqueue, stop_port, Shared, Work};
use crate::error::PortError;
use crate::ports::{AccountChange, CloudAvailability, CloudEvents};
use kudzu_core::sync::SyncIndex;
use std::collections::BTreeMap;
use std::sync::{Arc, PoisonError};
use std::time::Duration;

pub(super) struct Job {
    pub(super) index: SyncIndex,
    events: Arc<dyn CloudEvents>,
    pub(super) running: bool,
    applied: Option<bool>,
    pub(super) generation: u64,
    usage_credit: Duration,
    pub(super) force_usage: bool,
    pub(super) queued_send: bool,
    pub(super) pending_push: bool,
    start_failed: bool,
    permanent_unavailable: bool,
    pub(super) signed_out: bool,
    pub(super) availability: Option<CloudAvailability>,
    pub(super) pending_reset: Option<AccountChange>,
    pub(super) in_flight: BTreeMap<String, Flight>,
}

pub(super) enum Resume {
    Stored,
    Fresh,
}

impl Job {
    pub(super) fn new(index: SyncIndex, events: Arc<dyn CloudEvents>) -> Self {
        Self {
            index,
            events,
            running: false,
            applied: None,
            generation: 0,
            usage_credit: Duration::ZERO,
            force_usage: false,
            queued_send: false,
            pending_push: false,
            start_failed: false,
            permanent_unavailable: false,
            signed_out: false,
            availability: None,
            pending_reset: None,
            in_flight: BTreeMap::new(),
        }
    }

    pub(super) fn turn(&mut self, shared: &Arc<Shared>, work: Work) {
        if shared
            .panic_turn
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            panic!("sync worker test panic");
        }
        if let Some(enabled) = work.enabled {
            self.apply_enabled(shared, enabled);
        }
        for event in work.events {
            self.dispatch(shared, event);
        }
        if work.sync_now {
            self.sync_now(shared, false);
        }
        self.finish_uploads(shared);
    }

    fn apply_enabled(&mut self, shared: &Arc<Shared>, enabled: bool) {
        if self.applied == Some(enabled) {
            // Explicit set_enabled while running still fetches. Settings ignores unchanged values.
            if enabled && self.running {
                self.sync_now(shared, false);
            }
            return;
        }
        self.applied = Some(enabled);
        if !enabled {
            self.halt(shared);
            publish(shared, SyncStatus::Off);
            return;
        }
        if status::dev_gate(shared.production, true).is_some() {
            self.halt(shared);
            publish(
                shared,
                SyncStatus::Unavailable {
                    reason: DEV_UNAVAILABLE.into(),
                },
            );
            return;
        }
        self.permanent_unavailable = false;
        self.begin(shared, Resume::Stored);
    }

    /// Stop the port. Persist engine state only when the library revision is already durable.
    fn halt(&mut self, shared: &Arc<Shared>) {
        self.disarm(shared);
        self.generation = bump(shared);
        self.running = false;
        self.start_failed = false;
        self.pending_reset = None;
        self.signed_out = false;
        self.force_usage = false;
        self.queued_send = false;
        self.pending_push = false;
        let _ = super::apply::flush(shared);
        stop_port(shared);
    }

    pub(super) fn begin(&mut self, shared: &Arc<Shared>, resume: Resume) {
        let keep_signed_out = self.signed_out;
        if !keep_signed_out {
            publish(shared, SyncStatus::Starting);
        }
        self.disarm(shared);
        self.generation = bump(shared);
        self.usage_credit = Duration::ZERO;
        self.force_usage = false;
        self.queued_send = false;
        self.pending_push = false;
        self.in_flight.clear();
        self.running = false;
        self.start_failed = false;
        let checkpoint = match resume {
            Resume::Fresh => None,
            Resume::Stored => {
                let stored = shared.files.lock().unwrap_or_else(PoisonError::into_inner);
                files::read_checkpoint(&stored.checkpoint).unwrap_or(None)
            }
        };
        match shared
            .cloud
            .start(self.generation, checkpoint, self.events.clone())
        {
            Err(PortError::Unavailable) => {
                self.permanent_unavailable = true;
                publish(
                    shared,
                    SyncStatus::Unavailable {
                        reason: UNENTITLED.into(),
                    },
                );
            }
            Err(_) => {
                self.start_failed = true;
                publish(
                    shared,
                    SyncStatus::Error {
                        message: START_FAILED.into(),
                    },
                );
                self.arm_timer(shared);
            }
            Ok(()) => {
                self.running = true;
                shared
                    .port_started
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                self.arm_timer(shared);
                if keep_signed_out {
                    publish(
                        shared,
                        SyncStatus::NoAccount {
                            reason: status::SIGNED_OUT.into(),
                        },
                    );
                } else {
                    self.publish_live(
                        shared,
                        if self.index.blocked_by_newer_format() {
                            SyncStatus::NeedsUpdate
                        } else {
                            SyncStatus::Syncing
                        },
                    );
                }
                if shared.cloud.fetch(self.generation).is_err() && !keep_signed_out {
                    self.publish_live(
                        shared,
                        SyncStatus::Error {
                            message: TEMPORARILY_UNAVAILABLE.into(),
                        },
                    );
                }
            }
        }
    }

    pub(super) fn arm_timer(&mut self, shared: &Arc<Shared>) {
        let generation = self.generation;
        let weak = Arc::downgrade(shared);
        match shared
            .scheduler
            .every(status::FETCH_INTERVAL, status::FETCH_INTERVAL, move || {
                if let Some(shared) = weak.upgrade() {
                    enqueue(&shared, super::Inbound::Tick(generation));
                }
            }) {
            Ok(timer) => {
                *shared.timer.lock().unwrap_or_else(PoisonError::into_inner) = Some(timer);
            }
            Err(_) => self.publish_live(
                shared,
                SyncStatus::Error {
                    message: START_FAILED.into(),
                },
            ),
        }
    }

    fn disarm(&self, shared: &Shared) {
        *shared.timer.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    fn dispatch(&mut self, shared: &Arc<Shared>, event: super::Inbound) {
        if matches!(event, super::Inbound::Durable) {
            self.on_durable(shared);
            return;
        }
        let Some(generation) = event.generation() else {
            return;
        };
        if !self.relevant(generation) {
            return;
        }
        match event {
            super::Inbound::Fetched(_, batch) => self.on_fetched(shared, batch),
            super::Inbound::Sent(_, report) => self.on_sent(shared, report),
            super::Inbound::Account(_, change) => self.on_account(shared, change),
            super::Inbound::Availability(_, availability) => {
                self.on_availability(shared, availability)
            }
            super::Inbound::Failed(_, error) => self.on_failed(shared, error),
            super::Inbound::ZoneDeleted(_) => self.on_zone_deleted(shared),
            super::Inbound::State(_, bytes) => self.on_state(shared, bytes),
            super::Inbound::Tick(_) => self.on_tick(shared),
            super::Inbound::Durable => {}
        }
    }

    fn relevant(&self, generation: u64) -> bool {
        generation == self.generation
            && (self.running || self.start_failed || self.pending_reset.is_some())
    }

    fn on_tick(&mut self, shared: &Arc<Shared>) {
        if self.pending_reset.is_some() {
            self.try_reset(shared);
            return;
        }
        if !self.running {
            if self.start_failed && !self.permanent_unavailable {
                self.begin(shared, Resume::Stored);
            }
            return;
        }
        self.usage_credit += status::FETCH_INTERVAL;
        let include_usage = self.usage_credit >= status::USAGE_INTERVAL;
        if include_usage {
            self.usage_credit = Duration::ZERO;
        }
        self.sync_now(shared, include_usage);
    }

    fn sync_now(&mut self, shared: &Arc<Shared>, include_usage: bool) {
        if !self.running {
            if self.permanent_unavailable || self.pending_reset.is_some() {
                return;
            }
            if self.applied == Some(true) && status::dev_gate(shared.production, true).is_none() {
                self.begin(shared, Resume::Stored);
            }
            return;
        }
        if shared.cloud.fetch(self.generation).is_err() {
            if !self.blocks_progress() {
                self.publish_live(
                    shared,
                    SyncStatus::Error {
                        message: TEMPORARILY_UNAVAILABLE.into(),
                    },
                );
            }
            return;
        }
        if self.index.blocked_by_newer_format() {
            publish(shared, SyncStatus::NeedsUpdate);
            return;
        }
        if !self.blocks_progress() {
            publish(shared, SyncStatus::Syncing);
        }
        if self.index.initial_merge_done {
            self.queued_send = true;
            if include_usage {
                self.force_usage = true;
            }
        }
    }

    fn on_account(&mut self, shared: &Arc<Shared>, change: AccountChange) {
        if change == AccountChange::SignIn {
            self.signed_out = false;
            self.availability = None;
            if self.running && !self.index.blocked_by_newer_format() {
                publish(shared, SyncStatus::Syncing);
                self.sync_now(shared, false);
            }
            return;
        }
        self.disarm(shared);
        self.generation = bump(shared);
        self.running = false;
        self.start_failed = false;
        self.force_usage = false;
        self.queued_send = false;
        self.pending_push = false;
        self.in_flight.clear();
        stop_port(shared);
        self.pending_reset = Some(change);
        self.signed_out = change == AccountChange::SignOut;
        log::info!("sync: iCloud account changed; server knowledge reset");
        self.try_reset(shared);
    }

    fn on_availability(&mut self, shared: &Arc<Shared>, availability: CloudAvailability) {
        if availability == CloudAvailability::CouldNotDetermine {
            return;
        }
        if self.signed_out && availability == CloudAvailability::NoAccount {
            return;
        }
        if availability == CloudAvailability::Available {
            self.signed_out = false;
            self.availability = None;
            if self.index.blocked_by_newer_format() {
                publish(shared, SyncStatus::NeedsUpdate);
                return;
            }
            if self.running {
                self.sync_now(shared, false);
            }
            return;
        }
        self.availability = Some(availability);
        publish(shared, availability_status(availability));
    }

    fn on_failed(&mut self, shared: &Arc<Shared>, error: PortError) {
        if error == PortError::Unavailable {
            self.permanent_unavailable = true;
            publish(
                shared,
                SyncStatus::Unavailable {
                    reason: UNENTITLED.into(),
                },
            );
            return;
        }
        if self.blocks_progress() {
            return;
        }
        self.publish_live(
            shared,
            SyncStatus::Error {
                message: TEMPORARILY_UNAVAILABLE.into(),
            },
        );
    }

    pub(super) fn blocks_progress(&self) -> bool {
        self.signed_out || self.availability.is_some()
    }

    pub(super) fn publish_live(&self, shared: &Shared, status: SyncStatus) {
        if self.index.blocked_by_newer_format()
            && matches!(
                status,
                SyncStatus::Starting
                    | SyncStatus::Syncing
                    | SyncStatus::UpToDate { .. }
                    | SyncStatus::Error { .. }
            )
        {
            publish(shared, SyncStatus::NeedsUpdate);
            return;
        }
        publish(shared, status);
    }

    pub(super) fn publish_progress(&self, shared: &Shared, status: SyncStatus) {
        if self.blocks_progress()
            && matches!(
                status,
                SyncStatus::Starting
                    | SyncStatus::Syncing
                    | SyncStatus::UpToDate { .. }
                    | SyncStatus::Error { .. }
            )
        {
            return;
        }
        self.publish_live(shared, status);
    }
}

pub(super) fn publish(shared: &Shared, status: SyncStatus) {
    shared
        .status
        .replace(Arc::new(status))
        .notification
        .notify();
}

fn availability_status(availability: CloudAvailability) -> SyncStatus {
    match availability {
        CloudAvailability::NoAccount => SyncStatus::NoAccount {
            reason: status::NO_ACCOUNT.into(),
        },
        CloudAvailability::Restricted => SyncStatus::NoAccount {
            reason: status::RESTRICTED.into(),
        },
        CloudAvailability::TemporarilyUnavailable => SyncStatus::NoAccount {
            reason: TEMPORARILY_UNAVAILABLE.into(),
        },
        CloudAvailability::Unavailable => SyncStatus::Unavailable {
            reason: UNENTITLED.into(),
        },
        CloudAvailability::Available | CloudAvailability::CouldNotDetermine => SyncStatus::Syncing,
    }
}
