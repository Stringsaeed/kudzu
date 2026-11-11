//! Local clipboard capture and history. Clipboard content never enters logs or sync.
mod runtime;
mod search;
pub use search::AgentClip;
mod storage;
#[cfg(test)]
mod tests;

use crate::{
    error::{IoFailure, PortError},
    persist::{JsonFile, LoadSource, PersistError, Persisted, Revision, SaveState, Writer},
    ports::{AppCatalog, Clock, Pasteboard, TargetApp, Thumbnailer},
    settings::Settings,
    watch::{Publisher, Watch},
};
use kudzu_core::{
    clipboard::{self, classify, ClipEntry, ClipKind, ClipSource, History, ImageSize, PasteItem},
    system::AppInfo,
};
use std::{
    fs,
    io::Read,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Thumbnail,
    Image,
    File(usize),
    Icon(usize),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}
pub struct DeletedClips(Vec<ClipEntry>);
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

/// Call potentially blocking operations on a worker. `poll` is also exposed for deterministic fakes.
pub struct ClipboardService {
    state: Mutex<State>,
    store: Store,
    persisted: Persisted<History>,
    settings: Watch<Settings>,
    board: Arc<dyn Pasteboard>,
    thumbnails: Arc<dyn Thumbnailer>,
    target: Arc<dyn TargetApp>,
    apps: Arc<dyn AppCatalog>,
    clock: Arc<dyn Clock>,
    runtime: Mutex<Option<runtime::Runtime>>,
    search: Mutex<Option<search::SearchCache>>,
    #[cfg(test)]
    cache_builds: std::sync::atomic::AtomicUsize,
    storage_error: Publisher<Option<IoFailure>>,
}
impl ClipboardService {
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        dir: PathBuf,
        settings: Watch<Settings>,
        board: Arc<dyn Pasteboard>,
        thumbnails: Arc<dyn Thumbnailer>,
        target: Arc<dyn TargetApp>,
        apps: Arc<dyn AppCatalog>,
        clock: Arc<dyn Clock>,
        writer: &Writer,
    ) -> Result<Self, ClipboardError> {
        let store = Store::open(dir)?;
        let (file, mut loaded) = JsonFile::open(store.dir.join("history.json"), History::new)?;
        let durable = if loaded.source == LoadSource::Existing {
            Revision(1)
        } else {
            Revision(0)
        };
        let dirty = loaded.value.rescan_secrets();
        store.recover_jobs(&loaded.value)?;
        store.clean(&loaded.value)?;
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
        if !initial_cleanup.is_empty() {
            store.write_jobs(&initial_cleanup)?;
        }
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
        Ok(Self {
            state: Mutex::new(State {
                publisher: Publisher::with_revision(value, Revision(1)),
                last_change,
                retention,
                cleanup: initial_cleanup,
            }),
            store,
            persisted,
            settings,
            board,
            thumbnails,
            target,
            apps,
            clock,
            runtime: Mutex::new(None),
            search: Mutex::new(None),
            #[cfg(test)]
            cache_builds: std::sync::atomic::AtomicUsize::new(0),
            storage_error: Publisher::new(None),
        })
    }
    /// Local-only: snapshots contain unredacted clipboard data.
    pub fn watch(&self) -> Watch<History> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .publisher
            .watch()
    }
    pub fn saved(&self) -> Watch<SaveState> {
        self.persisted.saved()
    }
    fn change<T>(
        &self,
        edit: impl FnOnce(&mut History) -> Result<T, ClipboardError>,
    ) -> Result<T, ClipboardError> {
        let (value, notification) = {
            let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let (revision, current) = state.publisher.watch().snapshot();
            let mut next = (*current).clone();
            let value = edit(&mut next)?;
            if next == *current {
                return Ok(value);
            }
            let next = Arc::new(next);
            self.persisted
                .submit(Revision(revision.0 + 1), next.clone())?;
            (value, state.publisher.replace(next).notification)
        };
        notification.notify();
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
        let (result, notice) = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let (revision, current) = state.publisher.watch().snapshot();
            let mut next = (*current).clone();
            let (result, entries) = edit(&mut next);
            if entries.is_empty() {
                return Ok(result);
            }
            let revision = Revision(revision.0 + 1);
            let next = Arc::new(next);
            let mut jobs = state.cleanup.clone();
            jobs.push(Cleanup {
                revision,
                entries,
                trash,
            });
            self.store.write_jobs(&jobs)?;
            self.persisted.submit(revision, next.clone())?;
            state.cleanup = jobs;
            (result, state.publisher.replace(next).notification)
        };
        notice.notify();
        Ok(result)
    }

    /// Perform only cleanup whose removal is durable. Call on a worker, never on the UI thread.
    pub fn collect(&self) -> Result<(), ClipboardError> {
        let result = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let durable = self.persisted.saved().get().durable;
            let current = state.publisher.watch().get();
            let mut result = Ok(());
            let mut jobs = state.cleanup.clone();
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
            if result.is_ok() && jobs.len() != state.cleanup.len() {
                result = self.store.write_jobs(&jobs);
                if result.is_ok() {
                    state.cleanup = jobs;
                }
            }
            result
        };
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
    /// Checks the current count once; callers schedule this every 300 ms.
    pub fn poll(&self) -> Result<(), ClipboardError> {
        let settings = self.settings.get();
        let count = self.board.change_count();
        let (changed, retention_changed) = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let changed = count != state.last_change;
            state.last_change = count;
            let retention_changed = state.retention != settings.clipboard_retention_days;
            state.retention = settings.clipboard_retention_days;
            (changed, retention_changed)
        };
        if retention_changed {
            self.prune()?;
        }
        if !changed || !settings.clipboard_enabled {
            return Ok(());
        }
        let snapshot = self.board.read()?;
        if snapshot.own || clipboard::is_private(&snapshot.items) {
            return Ok(());
        }
        let front = self.target.frontmost()?;
        if front
            .as_ref()
            .is_some_and(|a| settings.clipboard_ignored_apps.contains(&a.bundle_id))
        {
            return Ok(());
        }
        let Some(c) = classify(
            &snapshot.items,
            front.as_ref().map(|a| a.bundle_id.as_str()),
        ) else {
            return Ok(());
        };
        let source = front.map(|a| ClipSource {
            bundle_id: a.bundle_id,
            name: a.name,
            path: a.path,
        });
        let thumb = match c.kind {
            ClipKind::Image => clipboard::image_of(&snapshot.items)
                .and_then(|(bytes, _)| self.thumbnails.thumbnail(bytes, 480).ok().flatten()),
            ClipKind::File
                if c.files.len() == 1 && clipboard::image_mime_for_path(&c.files[0]).is_some() =>
            {
                fs::metadata(&c.files[0])
                    .ok()
                    .filter(|m| m.is_file() && m.len() <= MAX_FILE_THUMB)
                    .and_then(|_| fs::read(&c.files[0]).ok())
                    .and_then(|bytes| self.thumbnails.thumbnail(&bytes, 480).ok().flatten())
            }
            _ => None,
        };
        let image = thumb.as_ref().map(|t| ImageSize {
            width: t.width,
            height: t.height,
        });
        self.change(|history| {
            let entry = history
                .ingest(&c, source, image, self.clock.now().into())
                .entry()
                .clone();
            if c.size <= MAX_BLOB {
                self.store.write(
                    &self.store.blob(entry.id),
                    &clipboard::encode_items(&snapshot.items),
                )?;
            } else {
                let _ = fs::remove_file(self.store.blob(entry.id));
            }
            if let Some(t) = &thumb {
                self.store.write(&self.store.thumb(entry.id), &t.png)?;
            }
            Ok(())
        })?;
        self.prune()
    }
    pub fn prune(&self) -> Result<(), ClipboardError> {
        let now = self.clock.now().into();
        let days = self.settings.get().clipboard_retention_days;
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
    pub fn history_text(&self, n: usize, current: Option<&str>) -> Option<String> {
        if !self.settings.get().clipboard_enabled {
            return None;
        }
        let h = self.watch().get();
        let newest = h.entries.first()?;
        let full_newest = self
            .store
            .items(newest.id)
            .as_deref()
            .and_then(clipboard::text_of)
            .or_else(|| {
                (newest.text_len <= clipboard::TEXT_EXCERPT_LIMIT)
                    .then(|| newest.text.clone())
                    .flatten()
            });
        let is_current = match (full_newest.as_deref(), current) {
            (Some(text), Some(current)) => text == current,
            (None, None) => self
                .board
                .read()
                .ok()
                .and_then(|snapshot| classify(&snapshot.items, None))
                .is_some_and(|copy| format!("{:016x}", copy.hash) == newest.hash),
            _ => false,
        };
        let index = if is_current { n } else { n.checked_sub(1)? };
        let e = h.entries.get(index)?;
        self.store
            .items(e.id)
            .as_deref()
            .and_then(clipboard::text_of)
            .or_else(|| e.text.clone())
    }
    pub fn pin(&self, id: Uuid, pinned: bool) -> Result<(), ClipboardError> {
        self.change(|h| {
            h.set_pinned(id, pinned).ok_or(ClipboardError::Missing)?;
            Ok(())
        })
    }
    pub fn set_secret(&self, id: Uuid, value: Option<bool>) -> Result<(), ClipboardError> {
        self.change(|h| {
            h.set_secret(id, value).ok_or(ClipboardError::Missing)?;
            Ok(())
        })
    }
    pub fn delete(&self, ids: &[Uuid]) -> Result<DeletedClips, ClipboardError> {
        self.remove_with(true, |h| {
            let entries = h.remove(ids);
            (DeletedClips(entries.clone()), entries)
        })
    }
    pub fn restore(&self, deleted: DeletedClips) -> Result<(), ClipboardError> {
        self.change(|h| {
            let restored = h.restore(deleted.0);
            self.store.trash(&restored, true)?;
            Ok(())
        })
    }
    pub fn clear(&self, include_pinned: bool) -> Result<usize, ClipboardError> {
        self.remove_with(false, |h| {
            let entries = h.clear(include_pinned);
            (entries.len(), entries)
        })
    }
    pub fn usage(&self) -> u64 {
        self.store.usage()
    }
    pub fn copy(&self, id: Uuid, flavor: Flavor) -> Result<(), ClipboardError> {
        let h = self.watch().get();
        let e = h.get(id).ok_or(ClipboardError::Missing)?;
        let stored = self.store.items(id);
        if stored.is_none() && e.size > MAX_BLOB {
            return Err(ClipboardError::TooLarge);
        }
        let full = stored.as_deref().and_then(clipboard::text_of).or_else(|| {
            (e.text_len <= clipboard::TEXT_EXCERPT_LIMIT)
                .then(|| e.text.clone())
                .flatten()
        });
        let plain = || {
            full.clone()
                .map(|s| vec![vec![(clipboard::uti::PLAIN_TEXT.into(), s.into_bytes())]])
        };
        let items = if flavor == Flavor::Plain {
            plain().or(stored)
        } else {
            stored.or_else(plain)
        }
        .ok_or(ClipboardError::TooLarge)?;
        self.board.write(&items)?;
        self.change(|h| {
            h.mark_used(id, self.clock.now().into())
                .ok_or(ClipboardError::Missing)?;
            Ok(())
        })
    }
    pub fn begin_pick(self: &Arc<Self>) -> Result<PickSession, ClipboardError> {
        Ok(PickSession {
            service: self.clone(),
            target: self.target.frontmost()?,
            can_paste: self.board.can_paste(),
        })
    }
    pub fn media(&self, id: Uuid, kind: MediaKind) -> Option<Media> {
        let h = self.watch().get();
        let e = h.get(id)?;
        let (mime, bytes) = match kind {
            MediaKind::Thumbnail => ("image/png", fs::read(self.store.thumb(id)).ok()?),
            MediaKind::Image => {
                let items = self.store.items(id)?;
                let (bytes, mime) = clipboard::image_of(&items)?;
                (mime, bytes.to_vec())
            }
            MediaKind::File(i) => {
                let path = e.files.get(i)?;
                let mime = clipboard::image_mime_for_path(path)?;
                let meta = fs::metadata(path).ok()?;
                if !meta.is_file() || meta.len() > MAX_IMAGE_FILE {
                    return None;
                }
                let mut bytes = Vec::new();
                fs::File::open(path)
                    .ok()?
                    .take(MAX_IMAGE_FILE + 1)
                    .read_to_end(&mut bytes)
                    .ok()?;
                if bytes.len() as u64 > MAX_IMAGE_FILE {
                    return None;
                }
                (mime, bytes)
            }
            MediaKind::Icon(i) => (
                "image/png",
                self.apps.icon(e.files.get(i)?, 64).ok().flatten()?,
            ),
        };
        Some(Media { mime, bytes })
    }
}
pub struct PickSession {
    service: Arc<ClipboardService>,
    target: Option<AppInfo>,
    pub can_paste: bool,
}
impl PickSession {
    pub fn paste(&self, id: Uuid, flavor: Flavor) -> Result<PasteOutcome, ClipboardError> {
        self.service.copy(id, flavor)?;
        if !self.service.board.can_paste() {
            return Ok(PasteOutcome::Copied);
        }
        if let Some(target) = &self.target {
            match self.service.target.activate(target) {
                Ok(()) => {}
                Err(PortError::TargetGone) => return Err(ClipboardError::TargetGone),
                Err(e) => return Err(e.into()),
            }
            let deadline = Instant::now() + Duration::from_millis(800);
            loop {
                let active = self.service.target.frontmost()?;
                if active
                    .as_ref()
                    .is_some_and(|app| match (target.pid, app.pid) {
                        (Some(expected), Some(actual)) => expected == actual,
                        _ => target.bundle_id == app.bundle_id,
                    })
                {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err(ClipboardError::TargetGone);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        match self.service.board.paste(self.target.as_ref()) {
            Ok(()) => Ok(PasteOutcome::Pasted),
            Err(PortError::PermissionDenied) => Ok(PasteOutcome::Copied),
            Err(PortError::TargetGone) => Err(ClipboardError::TargetGone),
            Err(e) => Err(e.into()),
        }
    }
}
