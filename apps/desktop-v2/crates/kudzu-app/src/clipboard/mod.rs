//! Local clipboard capture and history. Clipboard content never enters logs or sync.
mod capture;
mod media;
mod pick;
mod runtime;
pub use media::{Media, MediaKind};
pub use pick::PickSession;
mod search;
pub use search::{AgentItem, ClipResults, MAX_ITEM_CHARS};
#[cfg(test)]
mod review_tests;
mod storage;
#[cfg(test)]
mod tests;

use crate::{
    error::{IoFailure, PortError},
    persist::{JsonFile, LoadSource, PersistError, Persisted, Revision, SaveState, Writer},
    ports::{Clock, FileIcons, Pasteboard, TargetApp, Thumbnailer},
    settings::Settings,
    watch::{Publisher, Watch},
};
use kudzu_core::{
    clipboard::{self, classify, ClipEntry, ClipKind, ClipSource, History, ImageSize, PasteItem},
    system::AppInfo,
};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};
use storage::Store;
use uuid::Uuid;

const MAX_BLOB: u64 = 64 * 1024 * 1024;
const MAX_FILE_THUMB: u64 = 40 * 1024 * 1024;
const MAX_IMAGE_FILE: u64 = 80 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ClipboardError {
    #[error("That item is no longer in your clipboard history.")]
    Missing,
    #[error("This item was too large to keep, so it can't be copied again.")]
    TooLarge,
    #[error("The app you came from isn't available any more. The item is on your clipboard.")]
    TargetGone,
    #[error("The saved data for this clipboard item is missing.")]
    MissingData,
    #[error("The saved data for this clipboard item is damaged.")]
    CorruptData,
    #[error("Clipboard history storage failed.")]
    Storage(#[from] IoFailure),
    #[error("Clipboard history could not be saved.")]
    Persistence(#[from] PersistError),
    #[error("The clipboard operation failed.")]
    Port(#[from] PortError),
    #[error("The clipboard timer could not start.")]
    Schedule(crate::schedule::ScheduleError),
    #[error("The clipboard worker stopped unexpectedly.")]
    WorkerStopped,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    Original,
    Plain,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteOutcome {
    Pasted,
    Copied,
}
pub struct DeletedClips(Vec<ClipEntry>);
impl DeletedClips {
    pub fn entries(&self) -> &[ClipEntry] {
        &self.0
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
struct State {
    publisher: Publisher<History>,
    last_change: i64,
    retention: u32,
    cleanup: Vec<Cleanup>,
}
#[derive(Clone)]
struct Cleanup {
    revision: Revision,
    entries: Vec<ClipEntry>,
    trash: bool,
}

/// Ports used by the clipboard service; all operations run off the UI thread.
pub struct ClipboardPorts {
    pub board: Arc<dyn Pasteboard>,
    pub thumbnails: Arc<dyn Thumbnailer>,
    pub target: Arc<dyn TargetApp>,
    pub icons: Arc<dyn FileIcons>,
    pub clock: Arc<dyn Clock>,
}
/// Call potentially blocking operations on a worker. `poll` is also exposed for deterministic fakes.
pub struct ClipboardService {
    state: Mutex<State>,
    history: Watch<History>,
    operations: Mutex<()>,
    store: Store,
    persisted: Persisted<History>,
    settings: Watch<Settings>,
    board: Arc<dyn Pasteboard>,
    thumbnails: Arc<dyn Thumbnailer>,
    target: Arc<dyn TargetApp>,
    icons: Arc<dyn FileIcons>,
    clock: Arc<dyn Clock>,
    runtime: Mutex<Option<runtime::Runtime>>,
    search: Mutex<Option<search::SearchCache>>,
    #[cfg(test)]
    cache_builds: std::sync::atomic::AtomicUsize,
    storage_error: Publisher<Option<IoFailure>>,
    capture_error: Publisher<Option<ClipboardError>>,
    usage_cache: Mutex<Option<(Revision, u64)>>,
    retry_collect: Mutex<Option<Duration>>,
}
impl ClipboardService {
    pub fn open(
        dir: PathBuf,
        settings: Watch<Settings>,
        ports: ClipboardPorts,
        writer: &Writer,
    ) -> Result<Self, ClipboardError> {
        let ClipboardPorts {
            board,
            thumbnails,
            target,
            icons,
            clock,
        } = ports;
        let store = Store::open(dir)?;
        let (file, mut loaded) = JsonFile::open(store.dir.join("history.json"), History::new)?;
        let durable = if loaded.source == LoadSource::Existing {
            Revision(1)
        } else {
            Revision(0)
        };
        store.clean(&loaded.value)?;
        let mut dirty = loaded.value.rescan_secrets();
        // Older builds only scanned a prefix. Recheck long blobs on every open until a
        // later model revision can identify which histories were scanned completely.
        for entry in &mut loaded.value.entries {
            if entry.text_len <= clipboard::TEXT_EXCERPT_LIMIT
                || !matches!(
                    entry.kind,
                    ClipKind::Text | ClipKind::Code | ClipKind::Link | ClipKind::Other
                )
            {
                continue;
            }
            let full = store
                .items(entry.id)
                .as_deref()
                .and_then(clipboard::text_of);
            let secret = match full {
                Some(full) if full.chars().count() == entry.text_len => {
                    kudzu_core::secrets::scan(&full)
                        .first()
                        .map(|finding| finding.kind)
                }
                _ => entry
                    .secret
                    .or(Some(kudzu_core::secrets::SecretKind::Credential)),
            };
            dirty |= secret != entry.secret;
            entry.secret = secret;
        }
        let retention = settings.get().clipboard_retention_days;
        let expired = loaded.value.prune(clock.now().into(), retention);
        let initial_cleanup = if expired.is_empty() {
            vec![]
        } else {
            vec![Cleanup {
                revision: Revision(1),
                entries: expired,
                trash: false,
            }]
        };
        let persisted = writer.register_with_min_interval(
            file,
            if dirty || !initial_cleanup.is_empty() {
                Revision(0)
            } else {
                durable
            },
            Duration::from_millis(900),
        )?;
        let value = Arc::new(loaded.value);
        persisted.submit(Revision(1), value.clone())?;
        let last_change = board.change_count();
        let publisher = Publisher::with_revision(value, Revision(1));
        let history = publisher.watch();
        Ok(Self {
            state: Mutex::new(State {
                publisher,
                last_change,
                retention,
                cleanup: initial_cleanup,
            }),
            history,
            operations: Mutex::new(()),
            store,
            persisted,
            settings,
            board,
            thumbnails,
            target,
            icons,
            clock,
            runtime: Mutex::new(None),
            search: Mutex::new(None),
            #[cfg(test)]
            cache_builds: std::sync::atomic::AtomicUsize::new(0),
            storage_error: Publisher::new(None),
            capture_error: Publisher::new(None),
            usage_cache: Mutex::new(None),
            retry_collect: Mutex::new(None),
        })
    }
    /// Local-only: snapshots contain unredacted clipboard data.
    pub fn watch(&self) -> Watch<History> {
        self.history.clone()
    }
    pub fn saved(&self) -> Watch<SaveState> {
        self.persisted.saved()
    }
    fn change<T>(
        &self,
        edit: impl FnOnce(&mut History) -> Result<T, ClipboardError>,
    ) -> Result<T, ClipboardError> {
        let operation = self
            .operations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (revision, current) = self.history.snapshot();
        let mut next = (*current).clone();
        let value = edit(&mut next)?;
        if next == *current {
            return Ok(value);
        }
        let next = Arc::new(next);
        self.persisted
            .submit(Revision(revision.0 + 1), next.clone())?;
        let notice = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .publisher
            .replace(next)
            .notification;
        drop(operation);
        notice.notify();
        Ok(value)
    }
    /// Errors from deferred file moves or purges; the files remain recoverable and cleanup retries.
    pub fn storage_error(&self) -> Watch<Option<IoFailure>> {
        self.storage_error.watch()
    }

    fn remove_with<T>(
        &self,
        trash: bool,
        edit: impl FnOnce(&mut History) -> (T, Vec<ClipEntry>),
    ) -> Result<T, ClipboardError> {
        let operation = self
            .operations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (revision, current) = self.history.snapshot();
        let mut next = (*current).clone();
        let (result, entries) = edit(&mut next);
        if entries.is_empty() {
            return Ok(result);
        }
        let revision = Revision(revision.0 + 1);
        let next = Arc::new(next);
        self.persisted.submit(revision, next.clone())?;
        let notice = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.cleanup.push(Cleanup {
                revision,
                entries,
                trash,
            });
            state.publisher.replace(next).notification
        };
        drop(operation);
        notice.notify();
        Ok(result)
    }
    /// Last capture failure, with no clipboard content in its error.
    pub fn capture_error(&self) -> Watch<Option<ClipboardError>> {
        self.capture_error.watch()
    }
    fn publish_capture_error(&self, error: Option<ClipboardError>) {
        self.capture_error
            .replace(Arc::new(error))
            .notification
            .notify();
    }
    /// Perform only cleanup whose removal is durable. Call on a worker, never on the UI thread.
    pub fn collect(&self) -> Result<(), ClipboardError> {
        self.collect_inner(false)
    }
    /// Call after all producers stop and the shared writer drains, before exiting.
    pub fn finish_shutdown(&self) -> Result<(), ClipboardError> {
        self.collect_inner(true)
    }
    fn collect_inner(&self, force: bool) -> Result<(), ClipboardError> {
        let now = self.clock.monotonic();
        if !force
            && self
                .retry_collect
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .is_some_and(|retry| now < retry)
        {
            return Ok(());
        }
        let operation = self
            .operations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let durable = self.persisted.saved().get().durable;
        let current = self.history.get();
        let mut jobs = {
            let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if !state.cleanup.iter().any(|job| job.revision <= durable) {
                return Ok(());
            }
            state.cleanup.clone()
        };
        let mut result = Ok(());
        jobs.retain_mut(|job| {
            if job.revision > durable {
                return true;
            }
            job.entries.retain(|entry| current.get(entry.id).is_none());
            if job.entries.is_empty() {
                return false;
            }
            let attempted = if job.trash {
                self.store.trash(&job.entries, false)
            } else {
                self.store.purge(&job.entries)
            };
            if let Err(error) = attempted {
                result = Err(error);
                true
            } else {
                false
            }
        });
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .cleanup = jobs;
        *self
            .retry_collect
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = result
            .as_ref()
            .err()
            .map(|_| now.saturating_add(Duration::from_secs(1)));
        *self
            .usage_cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        drop(operation);
        let failure = match result {
            Err(ClipboardError::Storage(error)) => Some(error),
            _ => None,
        };
        self.storage_error
            .replace(Arc::new(failure))
            .notification
            .notify();
        result
    }
    pub fn prune(&self) -> Result<(), ClipboardError> {
        let now: chrono::DateTime<chrono::Utc> = self.clock.now().into();
        let days = self.settings.get().clipboard_retention_days;
        let history = self.watch().get();
        let cutoff = (days > 0).then(|| now - chrono::Duration::days(i64::from(days)));
        let mut unpinned = 0usize;
        let needs_work = history.entries.iter().any(|entry| {
            if entry.pinned {
                return false;
            }
            unpinned += 1;
            unpinned > clipboard::MAX_ENTRIES || cutoff.is_some_and(|c| entry.copied_at < c)
        });
        if !needs_work {
            return Ok(());
        }
        self.remove_with(false, |h| {
            let removed = h.prune(now, days);
            ((), removed)
        })
    }
    /// Local-only: the full text may contain secrets.
    pub fn text(&self, id: Uuid) -> Result<Option<String>, ClipboardError> {
        let h = self.watch().get();
        let e = h.get(id).ok_or(ClipboardError::Missing)?;
        Ok(self
            .store
            .items(id)
            .as_deref()
            .and_then(clipboard::text_of)
            .or_else(|| e.text.clone()))
    }
    fn history_entry<'a>(
        &self,
        history: &'a History,
        n: usize,
        current: Option<&str>,
    ) -> Option<&'a ClipEntry> {
        let newest = history.entries.first()?;
        let is_current = match current {
            Some(text) => {
                format!(
                    "{:016x}",
                    clipboard::fnv1a([b"t".as_slice(), text.as_bytes()])
                ) == newest.hash
            }
            None => self
                .board
                .read()
                .ok()
                .and_then(|s| classify(&s.items, None))
                .is_some_and(|c| format!("{:016x}", c.hash) == newest.hash),
        };
        history
            .entries
            .get(if is_current { n } else { n.checked_sub(1)? })
    }
    pub fn history_text(&self, n: usize, current: Option<&str>) -> Option<String> {
        if !self.settings.get().clipboard_enabled {
            return None;
        }
        let history = self.watch().get();
        let entry = self.history_entry(&history, n, current)?;
        self.store
            .items(entry.id)
            .as_deref()
            .and_then(clipboard::text_of)
            .or_else(|| entry.text.clone())
    }
    pub fn pin(&self, id: Uuid, pinned: bool) -> Result<(), ClipboardError> {
        let history = self.watch().get();
        if history.get(id).ok_or(ClipboardError::Missing)?.pinned == pinned {
            return Ok(());
        }
        self.change(|h| {
            h.set_pinned(id, pinned).ok_or(ClipboardError::Missing)?;
            Ok(())
        })
    }
    pub fn set_secret(&self, id: Uuid, value: Option<bool>) -> Result<(), ClipboardError> {
        let history = self.watch().get();
        if history
            .get(id)
            .ok_or(ClipboardError::Missing)?
            .secret_override
            == value
        {
            return Ok(());
        }
        self.change(|h| {
            h.set_secret(id, value).ok_or(ClipboardError::Missing)?;
            Ok(())
        })
    }
    pub fn delete(&self, ids: &[Uuid]) -> Result<DeletedClips, ClipboardError> {
        if !self
            .watch()
            .get()
            .entries
            .iter()
            .any(|e| ids.contains(&e.id))
        {
            return Ok(DeletedClips(vec![]));
        }
        self.remove_with(true, |h| {
            let entries = h.remove(ids);
            (DeletedClips(entries.clone()), entries)
        })
    }
    pub fn restore(&self, deleted: DeletedClips) -> Result<(), ClipboardError> {
        let operation = self
            .operations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (revision, current) = self.history.snapshot();
        let mut next = (*current).clone();
        let restored = next.restore(deleted.0);
        if restored.is_empty() {
            return Ok(());
        }
        self.store.trash(&restored, true)?;
        let next = Arc::new(next);
        self.persisted
            .submit(Revision(revision.0 + 1), next.clone())?;
        let notification = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .publisher
            .replace(next)
            .notification;
        drop(operation);
        notification.notify();
        Ok(())
    }
    pub fn clear(&self, include_pinned: bool) -> Result<usize, ClipboardError> {
        if !self
            .watch()
            .get()
            .entries
            .iter()
            .any(|e| include_pinned || !e.pinned)
        {
            return Ok(0);
        }
        self.remove_with(false, |h| {
            let entries = h.clear(include_pinned);
            (entries.len(), entries)
        })
    }
    pub fn usage(&self) -> u64 {
        let revision = self.history.revision();
        if let Some((cached, value)) = *self
            .usage_cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
        {
            if cached == revision {
                return value;
            }
        }
        let value = self.store.usage();
        *self
            .usage_cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some((revision, value));
        value
    }
}
