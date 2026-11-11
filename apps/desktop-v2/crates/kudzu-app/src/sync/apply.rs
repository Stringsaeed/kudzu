//! Merge, durability, and the in-flight send set.
//!
//! The index and engine state reach disk only once `library.saved().durable` covers the
//! revision they belong to. Nothing is pushed while that hold is outstanding. A crash then
//! relaunches from files that still agree, so a remote add is not deleted and a remote delete
//! is not uploaded.
//!
//! A local delete that is still in flight, followed by a remote modify of the same record,
//! brings the snippet back. Tauri did the same; the merge rule is unchanged.

use super::files::{self, SyncFiles};
use super::job::{publish, Job, Resume};
use super::status::{self, SyncStatus, PERMANENT_FAILURE, QUOTA, SAVE_FAILED};
use super::Shared;
use crate::library::LibraryError;
use crate::ports::{AccountChange, CloudBatch, CloudSend, CloudSendFailure};
use crate::watch::Revision;
use kudzu_core::library::model::KudzuLibrary;
use kudzu_core::sync::{
    apply_remote, encode, initial_merge, local_changes, record_deleted, record_saved, usage_name,
    LocalChanges, SyncIndex,
};
use std::sync::{Arc, MutexGuard, PoisonError};

#[derive(Clone, PartialEq, Eq)]
pub(super) enum Flight {
    Save(String),
    Delete,
}

#[derive(Clone, PartialEq)]
pub(super) struct Hold {
    revision: u64,
    index: SyncIndex,
    state: Option<Vec<u8>>,
}

struct Merged {
    index: SyncIndex,
    needs_update: bool,
}

impl Job {
    pub(super) fn on_durable(&mut self, shared: &Shared) {
        self.finish_uploads(shared);
    }

    pub(super) fn on_state(&mut self, shared: &Shared, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        let revision = shared.library.watch().revision().0;
        let mut slot = shared.hold.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(existing) = slot.as_mut() {
            existing.state = Some(bytes);
            if revision > existing.revision {
                existing.revision = revision;
                existing.index = self.index.clone();
            }
        } else {
            *slot = Some(Hold {
                revision,
                index: self.index.clone(),
                state: Some(bytes),
            });
        }
    }

    pub(super) fn on_zone_deleted(&mut self, shared: &Shared) {
        self.index = SyncIndex {
            initial_merge_done: true,
            ..SyncIndex::default()
        };
        self.in_flight.clear();
        *shared.hold.lock().unwrap_or_else(PoisonError::into_inner) = None;
        if !write_index(shared, &self.index) {
            self.publish_live(
                shared,
                SyncStatus::Error {
                    message: SAVE_FAILED.into(),
                },
            );
            return;
        }
        self.force_usage = true;
        log::info!("sync: zone deleted; local library kept");
    }

    pub(super) fn try_reset(&mut self, shared: &Arc<Shared>) {
        let Some(change) = self.pending_reset else {
            return;
        };
        self.index = SyncIndex::default();
        self.in_flight.clear();
        *shared.hold.lock().unwrap_or_else(PoisonError::into_inner) = None;
        let saved = {
            let files = lock_files(shared);
            let index_ok = files.write_index(&self.index).is_ok();
            let checkpoint_ok = files::remove_checkpoint(&files.checkpoint).is_ok();
            index_ok && checkpoint_ok
        };
        if !saved {
            self.publish_live(
                shared,
                SyncStatus::Error {
                    message: SAVE_FAILED.into(),
                },
            );
            self.arm_timer(shared);
            return;
        }
        *disk_index(shared) = self.index.clone();
        self.pending_reset = None;
        self.signed_out = change == AccountChange::SignOut;
        self.availability = None;
        self.begin(shared, Resume::Fresh);
    }

    pub(super) fn on_fetched(&mut self, shared: &Shared, batch: CloudBatch) {
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
            Ok::<_, LibraryError>(Merged {
                index,
                needs_update: applied.needs_update,
            })
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
        let blocked = self.index.blocked_by_newer_format() || merged.needs_update;
        if full_fetch && !was_done && !blocked {
            self.force_usage = true;
        }
        if blocked {
            self.force_usage = false;
            publish(shared, SyncStatus::NeedsUpdate);
            return;
        }
        // A partial batch stays Syncing. Up to date waits for the fetch that finished.
        if full_fetch && !self.will_send(shared) {
            self.publish_progress(
                shared,
                SyncStatus::UpToDate {
                    last_sync: shared.clock.now(),
                },
            );
        }
    }

    pub(super) fn on_sent(&mut self, shared: &Shared, report: CloudSend) {
        for record in &report.saved {
            self.in_flight.remove(&record.name);
            record_saved(&mut self.index, record);
        }
        for name in &report.deleted {
            self.in_flight.remove(name);
            record_deleted(&mut self.index, name);
        }
        for record in &report.conflicts {
            self.in_flight.remove(&record.name);
        }
        for name in &report.requeue {
            self.in_flight.remove(name);
        }
        if report.zone_missing {
            self.in_flight.clear();
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
        if report.failure.is_none()
            && (report.zone_missing || !report.requeue.is_empty() || !report.conflicts.is_empty())
        {
            self.force_usage = true;
        }
        if self.index.blocked_by_newer_format() {
            publish(shared, SyncStatus::NeedsUpdate);
            return;
        }
        if let Some(failure) = report.failure {
            let message = match failure {
                CloudSendFailure::QuotaExceeded => QUOTA,
                CloudSendFailure::Permanent => PERMANENT_FAILURE,
            };
            self.publish_progress(
                shared,
                SyncStatus::Error {
                    message: message.into(),
                },
            );
            return;
        }
        if self.force_usage {
            self.publish_progress(shared, SyncStatus::Syncing);
        } else {
            self.publish_progress(
                shared,
                SyncStatus::UpToDate {
                    last_sync: shared.clock.now(),
                },
            );
        }
    }

    pub(super) fn finish_uploads(&mut self, shared: &Shared) {
        // An account reset writes its own empty index. Pushing here would publish that index
        // through the durability hold and leave `settle` waiting on a write that already failed.
        if self.pending_reset.is_some() {
            return;
        }
        let dirty = take_dirty(shared);
        let revision = shared.library.watch().revision().0;
        if dirty || self.index != *disk_index(shared) {
            raise_hold(shared, revision, &self.index);
        }
        if !flush(shared) {
            if dirty || self.queued_send || self.force_usage || self.pending_push {
                self.pending_push = true;
            }
            return;
        }
        let push = dirty || self.queued_send || self.force_usage || self.pending_push;
        if !push {
            return;
        }
        let include_usage = self.force_usage;
        let allow_empty = self.queued_send;
        self.force_usage = false;
        self.queued_send = false;
        self.pending_push = false;
        self.push(shared, include_usage, allow_empty);
    }

    fn will_send(&self, shared: &Shared) -> bool {
        if self.queued_send || self.pending_push {
            return true;
        }
        let library = shared.library.watch().get();
        !outbound(&library, &self.index, &shared.device_id, self.force_usage).is_empty()
    }

    fn push(&mut self, shared: &Shared, include_usage: bool, allow_empty: bool) {
        if !self.running || !self.index.initial_merge_done || self.index.blocked_by_newer_format() {
            return;
        }
        // One snapshot so an edit between the diff and the encode cannot drop a record.
        let library = shared.library.watch().get();
        let changes = outbound(&library, &self.index, &shared.device_id, include_usage);
        let records = changes
            .saves
            .iter()
            .filter(|name| !self.same_inflight_save(name, &library, shared))
            .filter_map(|name| encode(name, &library, &self.index, &shared.device_id))
            .collect::<Vec<_>>();
        let deleted = changes
            .deletes
            .into_iter()
            .filter(|name| !matches!(self.in_flight.get(name), Some(Flight::Delete)))
            .collect::<Vec<_>>();
        if records.is_empty() && deleted.is_empty() && !allow_empty {
            return;
        }
        let batch = CloudBatch {
            records,
            deleted,
            full_fetch: false,
        };
        let empty = batch.records.is_empty() && batch.deleted.is_empty();
        match shared.cloud.send(self.generation, batch.clone()) {
            Ok(()) => {
                for record in &batch.records {
                    self.in_flight
                        .insert(record.name.clone(), Flight::Save(record.payload.clone()));
                }
                for name in &batch.deleted {
                    self.in_flight.insert(name.clone(), Flight::Delete);
                }
                if empty {
                    self.publish_progress(
                        shared,
                        SyncStatus::UpToDate {
                            last_sync: shared.clock.now(),
                        },
                    );
                } else {
                    self.publish_progress(shared, SyncStatus::Syncing);
                }
            }
            Err(_) => {
                self.pending_push = true;
                self.publish_progress(
                    shared,
                    SyncStatus::Error {
                        message: status::TEMPORARILY_UNAVAILABLE.into(),
                    },
                );
            }
        }
    }

    fn same_inflight_save(&self, name: &str, library: &KudzuLibrary, shared: &Shared) -> bool {
        let Some(record) = encode(name, library, &self.index, &shared.device_id) else {
            return true;
        };
        matches!(self.in_flight.get(name), Some(Flight::Save(payload)) if payload == &record.payload)
    }
}

/// Writes the held index and engine state when the library revision is durable.
/// `false` means a push must wait.
pub(super) fn flush(shared: &Shared) -> bool {
    let saved = shared.library.saved().get();
    let hold = shared
        .hold
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let Some(hold) = hold else {
        return saved.error.is_none();
    };
    if saved.error.is_some() || saved.durable < Revision(hold.revision) {
        return false;
    }
    if hold.index != *disk_index(shared) && !write_index(shared, &hold.index) {
        publish(
            shared,
            SyncStatus::Error {
                message: SAVE_FAILED.into(),
            },
        );
        return false;
    }
    if let Some(bytes) = hold.state.as_ref().filter(|bytes| !bytes.is_empty()) {
        let path = lock_files(shared).checkpoint.clone();
        if files::write_checkpoint(&path, bytes).is_err() {
            publish(
                shared,
                SyncStatus::Error {
                    message: SAVE_FAILED.into(),
                },
            );
            return false;
        }
    }
    let mut slot = shared.hold.lock().unwrap_or_else(PoisonError::into_inner);
    if slot.as_ref() == Some(&hold) {
        *slot = None;
    }
    true
}

fn raise_hold(shared: &Shared, revision: u64, index: &SyncIndex) {
    let saved = shared.library.saved().get();
    let pending_index = index != &*disk_index(shared);
    let pending_library = saved.error.is_some() || saved.durable < Revision(revision);
    let mut slot = shared.hold.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(existing) = slot.as_mut() {
        if revision > existing.revision {
            existing.revision = revision;
        }
        existing.index = index.clone();
        return;
    }
    if pending_index || pending_library {
        *slot = Some(Hold {
            revision,
            index: index.clone(),
            state: None,
        });
    }
}

fn write_index(shared: &Shared, index: &SyncIndex) -> bool {
    let files = lock_files(shared);
    match files.write_index(index) {
        Ok(()) => {
            drop(files);
            *disk_index(shared) = index.clone();
            true
        }
        Err(_) => false,
    }
}

fn outbound(
    library: &KudzuLibrary,
    index: &SyncIndex,
    device_id: &str,
    include_usage: bool,
) -> LocalChanges {
    let mut changes = local_changes(library, index, device_id);
    if !include_usage {
        let own = usage_name(device_id);
        changes.saves.retain(|name| name != &own);
    }
    changes
}

fn take_dirty(shared: &Shared) -> bool {
    let mut inbox = shared.inbox.lock().unwrap_or_else(PoisonError::into_inner);
    let dirty = inbox.library_dirty;
    inbox.library_dirty = false;
    dirty
}

fn lock_files(shared: &Shared) -> MutexGuard<'_, SyncFiles> {
    shared.files.lock().unwrap_or_else(PoisonError::into_inner)
}

fn disk_index(shared: &Shared) -> MutexGuard<'_, SyncIndex> {
    shared
        .disk_index
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}
