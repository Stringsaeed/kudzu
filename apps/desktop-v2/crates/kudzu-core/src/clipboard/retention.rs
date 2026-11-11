//! The retention window and the entry cap.

use chrono::{DateTime, Duration, Utc};

use super::{ClipEntry, History, MAX_ENTRIES};

impl ClipEntry {
    /// When retention removes this entry (`None` when pinned or kept forever).
    pub fn expires_at(&self, retention_days: u32) -> Option<DateTime<Utc>> {
        if self.pinned || retention_days == 0 {
            None
        } else {
            Some(self.copied_at + Duration::days(i64::from(retention_days)))
        }
    }
}

impl History {
    /// Removes unpinned entries older than `retention_days` (0 keeps them) and any beyond
    /// [`MAX_ENTRIES`] unpinned ones. Returns what was removed.
    pub fn prune(&mut self, now: DateTime<Utc>, retention_days: u32) -> Vec<ClipEntry> {
        let cutoff = (retention_days > 0).then(|| now - Duration::days(i64::from(retention_days)));
        let mut kept_unpinned = 0usize;
        let mut removed = Vec::new();
        let mut kept = Vec::with_capacity(self.entries.len());
        for e in self.entries.drain(..) {
            let expired = !e.pinned && cutoff.is_some_and(|c| e.copied_at < c);
            let over = !e.pinned && kept_unpinned >= MAX_ENTRIES;
            if expired || over {
                removed.push(e);
            } else {
                if !e.pinned {
                    kept_unpinned += 1;
                }
                kept.push(e);
            }
        }
        self.entries = kept;
        removed
    }
}
