//! Clipboard history reads, mutations, and usage.
use super::*;

impl ClipboardService {
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
    pub(super) fn history_entry<'a>(
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
    /// Replaces the whole history in one commit. For headless scenes and tests, which have no
    /// pasteboard to copy from; the screens never call it.
    pub fn replace_history(&self, history: History) -> Result<(), ClipboardError> {
        self.change(|h| {
            *h = history;
            Ok(())
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
