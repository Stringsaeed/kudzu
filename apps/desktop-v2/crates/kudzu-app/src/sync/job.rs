//! One serialised sync turn: merge inside `library.transact`, then the index, then sends.
//! Callbacks from an older generation are ignored. Nothing here runs on the UI thread.

use super::files::{self, SyncFiles};
use super::status::{
    self, SyncStatus, DEV_UNAVAILABLE, PERMANENT_FAILURE, QUOTA, SAVE_FAILED, START_FAILED,
    TEMPORARILY_UNAVAILABLE, UNENTITLED,
};
use super::{bump, enqueue, Shared, Work};
use crate::error::PortError;
use crate::library::LibraryError;
use crate::ports::{CloudAvailability, CloudBatch, CloudEvents, CloudSend, CloudSendFailure};
use kudzu_core::sync::{
    apply_remote, encode, initial_merge, local_changes, record_deleted, record_saved, usage_name,
    Applied, SyncIndex,
};
use std::sync::{Arc, PoisonError};
use std::time::Duration;

pub(super) struct Job {
    index: SyncIndex,
    saved: SyncIndex,
    files: SyncFiles,
    events: Arc<dyn CloudEvents>,
    running: bool,
    applied: Option<bool>,
    generation: u64,
    usage_credit: Duration,
    force_usage: bool,
    queued_send: bool,
}

struct Merged {
    index: SyncIndex,
    applied: Applied,
}

enum Resume {
    Stored,
    Fresh,
}

impl Job {
    pub(super) fn new(index: SyncIndex, files: SyncFiles, events: Arc<dyn CloudEvents>) -> Self {
        Self {
            saved: index.clone(),
            index,
            files,
            events,
            running: false,
            applied: None,
            generation: 0,
            usage_credit: Duration::ZERO,
            force_usage: false,
            queued_send: false,
        }
    }

    pub(super) fn turn(&mut self, shared: &Arc<Shared>, work: Work) {
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
            if enabled && self.running {
                self.sync_now(shared, false);
            }
            return;
        }
        self.applied = Some(enabled);
        if !enabled {
            self.stop_running(shared, SyncStatus::Off);
            return;
        }
        if status::dev_gate(shared.production, true).is_some() {
            self.stop_running(
                shared,
                SyncStatus::Unavailable {
                    reason: DEV_UNAVAILABLE.into(),
                },
            );
            return;
        }
        self.begin(shared, Resume::Stored);
    }

    fn stop_running(&mut self, shared: &Arc<Shared>, status: SyncStatus) {
        self.halt(shared);
        publish(shared, status);
    }

    fn halt(&mut self, shared: &Arc<Shared>) {
        self.disarm(shared);
        self.generation = bump(shared);
        self.running = false;
        self.force_usage = false;
        self.queued_send = false;
        shared.cloud.stop();
    }

    fn begin(&mut self, shared: &Arc<Shared>, resume: Resume) {
        publish(shared, SyncStatus::Starting);
        self.disarm(shared);
        self.generation = bump(shared);
        self.usage_credit = Duration::ZERO;
        self.force_usage = false;
        self.queued_send = false;
        self.running = false;
        let checkpoint = match resume {
            Resume::Fresh => None,
            Resume::Stored => files::read_checkpoint(&self.files.checkpoint).unwrap_or(None),
        };
        match shared
            .cloud
            .start(self.generation, checkpoint, self.events.clone())
        {
            Err(PortError::Unavailable) => publish(
                shared,
                SyncStatus::Unavailable {
                    reason: UNENTITLED.into(),
                },
            ),
            Err(_) => publish(
                shared,
                SyncStatus::Error {
                    message: START_FAILED.into(),
                },
            ),
            Ok(()) => {
                self.running = true;
                self.arm_timer(shared);
                self.publish_live(
                    shared,
                    if self.index.blocked_by_newer_format() {
                        SyncStatus::NeedsUpdate
                    } else {
                        SyncStatus::Syncing
                    },
                );
                if shared.cloud.fetch(self.generation).is_err() {
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

    fn arm_timer(&mut self, shared: &Arc<Shared>) {
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
        if !self.fresh(event.generation()) {
            return;
        }
        match event {
            super::Inbound::Fetched(_, batch) => self.on_fetched(shared, batch),
            super::Inbound::Sent(_, report) => self.on_sent(shared, report),
            super::Inbound::Account(_) => self.on_account(shared),
            super::Inbound::Availability(_, availability) => {
                self.on_availability(shared, availability)
            }
            super::Inbound::Failed(_, error) => self.on_failed(shared, error),
            super::Inbound::ZoneDeleted(_) => self.on_zone_deleted(shared),
            super::Inbound::Tick(_) => self.on_tick(shared),
        }
    }

    fn fresh(&self, generation: u64) -> bool {
        self.running && generation == self.generation
    }

    fn on_tick(&mut self, shared: &Arc<Shared>) {
        self.usage_credit += status::FETCH_INTERVAL;
        if self.usage_credit >= status::USAGE_INTERVAL {
            self.usage_credit = Duration::ZERO;
            self.sync_now(shared, true);
        } else {
            self.sync_now(shared, false);
        }
    }

    fn sync_now(&mut self, shared: &Arc<Shared>, include_usage: bool) {
        if !self.running {
            if self.applied == Some(true) && status::dev_gate(shared.production, true).is_none() {
                self.begin(shared, Resume::Stored);
            }
            return;
        }
        if shared.cloud.fetch(self.generation).is_err() {
            self.publish_live(
                shared,
                SyncStatus::Error {
                    message: TEMPORARILY_UNAVAILABLE.into(),
                },
            );
            return;
        }
        if self.index.blocked_by_newer_format() {
            publish(shared, SyncStatus::NeedsUpdate);
            return;
        }
        publish(shared, SyncStatus::Syncing);
        if self.index.initial_merge_done {
            self.queued_send = true;
            if include_usage {
                self.force_usage = true;
            }
        }
    }

    fn on_account(&mut self, shared: &Arc<Shared>) {
        self.halt(shared);
        self.index = SyncIndex::default();
        let _ = self.commit_index(shared);
        if files::remove_checkpoint(&self.files.checkpoint).is_err() {
            self.publish_live(
                shared,
                SyncStatus::Error {
                    message: SAVE_FAILED.into(),
                },
            );
        }
        log::info!("sync: iCloud account changed; server knowledge reset");
        self.begin(shared, Resume::Fresh);
    }

    fn on_zone_deleted(&mut self, shared: &Arc<Shared>) {
        self.index = SyncIndex {
            initial_merge_done: true,
            ..SyncIndex::default()
        };
        if !self.commit_index(shared) {
            return;
        }
        self.force_usage = true;
        log::info!("sync: zone deleted; local library kept");
    }

    fn on_fetched(&mut self, shared: &Arc<Shared>, batch: CloudBatch) {
        log::info!(
            "sync: fetched {} records, {} deletions",
            batch.records.len(),
            batch.deleted.len()
        );
        let was_done = self.index.initial_merge_done;
        let device_id = shared.device_id.clone();
        let prior = self.index.clone();
        let full_fetch = batch.full_fetch;
        let records = batch.records;
        let deleted = batch.deleted;
        let merged = shared.library.transact(move |library| {
            let mut index = prior;
            let applied = apply_remote(library, &mut index, &records, &deleted, &device_id);
            if full_fetch {
                initial_merge(library, &mut index);
            }
            Ok::<_, LibraryError>(Merged { index, applied })
        });
        let Ok((merged, _)) = merged else {
            self.publish_live(
                shared,
                SyncStatus::Error {
                    message: SAVE_FAILED.into(),
                },
            );
            return;
        };
        self.index = merged.index;
        let blocked = self.index.blocked_by_newer_format() || merged.applied.needs_update;
        if full_fetch && !was_done && !blocked {
            self.force_usage = true;
        }
        if !self.commit_index(shared) {
            return;
        }
        self.store_engine(shared);
        if blocked {
            self.force_usage = false;
            publish(shared, SyncStatus::NeedsUpdate);
            return;
        }
        if !self.will_send(shared) {
            self.publish_live(
                shared,
                SyncStatus::UpToDate {
                    last_sync: shared.clock.now(),
                },
            );
        }
    }

    fn on_sent(&mut self, shared: &Arc<Shared>, report: CloudSend) {
        for record in &report.saved {
            record_saved(&mut self.index, record);
        }
        for name in &report.deleted {
            record_deleted(&mut self.index, name);
        }
        if !report.conflicts.is_empty() {
            let device_id = shared.device_id.clone();
            let prior = self.index.clone();
            let conflicts = report.conflicts.clone();
            let merged = shared.library.transact(move |library| {
                let mut index = prior;
                apply_remote(library, &mut index, &conflicts, &[], &device_id);
                Ok::<_, LibraryError>(index)
            });
            match merged {
                Ok((index, _)) => self.index = index,
                Err(_) => {
                    self.publish_live(
                        shared,
                        SyncStatus::Error {
                            message: SAVE_FAILED.into(),
                        },
                    );
                    return;
                }
            }
        }
        if !self.commit_index(shared) {
            return;
        }
        if report.zone_missing || !report.requeue.is_empty() || !report.conflicts.is_empty() {
            self.force_usage = true;
        }
        self.store_engine(shared);
        if self.index.blocked_by_newer_format() {
            publish(shared, SyncStatus::NeedsUpdate);
            return;
        }
        if let Some(failure) = report.failure {
            let message = match failure {
                CloudSendFailure::QuotaExceeded => QUOTA,
                CloudSendFailure::Permanent => PERMANENT_FAILURE,
            };
            self.publish_live(
                shared,
                SyncStatus::Error {
                    message: message.into(),
                },
            );
            return;
        }
        if self.force_usage {
            publish(shared, SyncStatus::Syncing);
        } else {
            self.publish_live(
                shared,
                SyncStatus::UpToDate {
                    last_sync: shared.clock.now(),
                },
            );
        }
    }

    fn on_availability(&mut self, shared: &Arc<Shared>, availability: CloudAvailability) {
        if self.index.blocked_by_newer_format() && availability == CloudAvailability::Available {
            publish(shared, SyncStatus::NeedsUpdate);
            return;
        }
        let status = match availability {
            CloudAvailability::Available => SyncStatus::Syncing,
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
        };
        publish(shared, status);
    }

    fn on_failed(&mut self, shared: &Arc<Shared>, error: PortError) {
        if error == PortError::Unavailable {
            publish(
                shared,
                SyncStatus::Unavailable {
                    reason: UNENTITLED.into(),
                },
            );
            return;
        }
        self.publish_live(
            shared,
            SyncStatus::Error {
                message: TEMPORARILY_UNAVAILABLE.into(),
            },
        );
    }

    fn finish_uploads(&mut self, shared: &Arc<Shared>) {
        let saved_ok = self.commit_index(shared);
        let dirty = take_dirty(shared);
        if !saved_ok {
            return;
        }
        let include_usage = self.force_usage;
        let allow_empty = self.queued_send;
        if !dirty && !include_usage && !allow_empty {
            return;
        }
        self.force_usage = false;
        self.queued_send = false;
        self.push(shared, include_usage, allow_empty);
    }

    fn push(&mut self, shared: &Arc<Shared>, include_usage: bool, allow_empty: bool) {
        if !self.running || !self.index.initial_merge_done || self.index.blocked_by_newer_format() {
            return;
        }
        let library = shared.library.watch().get();
        let changes = outbound(shared, &self.index, include_usage);
        if changes.is_empty() && !allow_empty {
            return;
        }
        let records = changes
            .saves
            .iter()
            .filter_map(|name| encode(name, &library, &self.index, &shared.device_id))
            .collect::<Vec<_>>();
        let batch = CloudBatch {
            records,
            deleted: changes.deletes,
            full_fetch: false,
        };
        let empty = batch.records.is_empty() && batch.deleted.is_empty();
        match shared.cloud.send(self.generation, batch) {
            Ok(()) if empty => self.publish_live(
                shared,
                SyncStatus::UpToDate {
                    last_sync: shared.clock.now(),
                },
            ),
            Ok(()) => publish(shared, SyncStatus::Syncing),
            Err(_) => self.publish_live(
                shared,
                SyncStatus::Error {
                    message: TEMPORARILY_UNAVAILABLE.into(),
                },
            ),
        }
    }

    fn will_send(&self, shared: &Shared) -> bool {
        if self.queued_send {
            return true;
        }
        !outbound(shared, &self.index, self.force_usage).is_empty()
    }

    fn commit_index(&mut self, shared: &Shared) -> bool {
        if self.index == self.saved {
            return true;
        }
        match self.files.write_index(&self.index) {
            Ok(()) => {
                self.saved = self.index.clone();
                true
            }
            Err(_) => {
                self.publish_live(
                    shared,
                    SyncStatus::Error {
                        message: SAVE_FAILED.into(),
                    },
                );
                false
            }
        }
    }

    fn store_engine(&self, shared: &Shared) {
        if let Ok(bytes) = shared.cloud.checkpoint() {
            let _ = files::write_checkpoint(&self.files.checkpoint, &bytes);
        }
    }

    fn publish_live(&self, shared: &Shared, status: SyncStatus) {
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
}

fn publish(shared: &Shared, status: SyncStatus) {
    shared
        .status
        .replace(Arc::new(status))
        .notification
        .notify();
}

fn take_dirty(shared: &Shared) -> bool {
    let mut inbox = shared.inbox.lock().unwrap_or_else(PoisonError::into_inner);
    let dirty = inbox.library_dirty;
    inbox.library_dirty = false;
    dirty
}

fn outbound(
    shared: &Shared,
    index: &SyncIndex,
    include_usage: bool,
) -> kudzu_core::sync::LocalChanges {
    let library = shared.library.watch().get();
    let mut changes = local_changes(&library, index, &shared.device_id);
    if !include_usage {
        let own = usage_name(&shared.device_id);
        changes.saves.retain(|name| name != &own);
    }
    changes
}
